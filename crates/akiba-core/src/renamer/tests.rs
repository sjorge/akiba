use std::cell::RefCell;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use anidb_udp::file::FileReply;
use anidb_udp::mask::{AnimeField, FileField, Value};
use anidb_udp::mylist::MylistEntry;
use serial_test::serial;
use tempfile::TempDir;

use super::names::truncate_name;
use super::*;

const FORMAT: &str = "{anime_name_romaji}/{episode:number}.{filetype}";
/// ed2k of the three bytes "abc" (a single block is its md4).
const ABC_ED2K: &str = "a448017aaf21d8525fc10ae87aa6729d";

type FileHandler = Box<dyn FnMut() -> std::result::Result<FileInfo, AnidbError>>;
type MylistHandler = Box<dyn FnMut() -> std::result::Result<MylistEntry, AnidbError>>;
type AddHandler = Box<dyn FnMut(MylistAdd) -> std::result::Result<ResponseCode, AnidbError>>;

#[derive(Default)]
struct Calls {
    file: usize,
    mylist: usize,
    adds: Vec<MylistAdd>,
    disconnects: usize,
}

struct Fake {
    calls: Rc<RefCell<Calls>>,
    file: FileHandler,
    mylist: MylistHandler,
    add: AddHandler,
}

fn reply_error(code: ResponseCode) -> AnidbError {
    AnidbError::Reply {
        code,
        message: code.to_string(),
    }
}

impl Fake {
    fn new() -> (Self, Rc<RefCell<Calls>>) {
        let calls = Rc::new(RefCell::new(Calls::default()));
        let fake = Self {
            calls: Rc::clone(&calls),
            file: Box::new(|| Err(reply_error(ResponseCode::NoSuchFile))),
            mylist: Box::new(|| Err(reply_error(ResponseCode::NoSuchEntry))),
            add: Box::new(|_| Ok(ResponseCode::MylistEntryAdded)),
        };
        (fake, calls)
    }
}

impl AnidbFiles for Fake {
    fn file_by_hash(&mut self, _: &str, _: u64) -> std::result::Result<FileInfo, AnidbError> {
        self.calls.borrow_mut().file += 1;
        (self.file)()
    }

    fn mylist_by_hash(&mut self, _: &str, _: u64) -> std::result::Result<MylistEntry, AnidbError> {
        self.calls.borrow_mut().mylist += 1;
        (self.mylist)()
    }

    fn mylist_add_by_hash(
        &mut self,
        _: &str,
        _: u64,
        options: MylistAdd,
    ) -> std::result::Result<ResponseCode, AnidbError> {
        self.calls.borrow_mut().adds.push(options);
        (self.add)(options)
    }

    fn disconnect(&mut self) {
        self.calls.borrow_mut().disconnects += 1;
    }
}

fn sample(overrides: impl FnOnce(&mut FileInfo)) -> FileInfo {
    let mut info = FileInfo {
        fid: 1_628_637,
        aid: 10_914,
        filetype: "mkv".to_owned(),
        anime_name_romaji: "Test Anime".to_owned(),
        episode: "013".to_owned(),
        episode_name: "Name".to_owned(),
        crc32: "438a7b93".to_owned(),
        md5: "m".to_owned(),
        sha1: "s".to_owned(),
        ..FileInfo::default()
    };
    overrides(&mut info);
    info
}

struct Fx {
    dir: TempDir,
    config: Config,
    source: PathBuf,
    target: PathBuf,
}

impl Fx {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        // NOTE: the cwd is a real path, /var is a link to /private/var
        let root = fs::canonicalize(dir.path()).expect("canonicalize");
        let target = root.join("target");
        fs::create_dir(&target).expect("target dir");
        let source = root.join("source.mkv");
        fs::write(&source, "abc").expect("source");

        let mut config = Config::default();
        config.cache.path = root.join("cache");
        config.anidb.udp_client.name = Some("client".to_owned());
        config.anidb.udp_client.version = Some(1);
        config.anidb.udp_client.username = Some("u".to_owned());
        config.anidb.udp_client.password = Some("p".to_owned());
        Self {
            dir,
            config,
            source,
            target,
        }
    }

    fn destination(&self) -> PathBuf {
        self.target.join("Test Anime").join("13.mkv")
    }

    fn renamer_with(&self, format: &str, target: &Path, fake: Fake) -> Renamer {
        Renamer::with_client(
            &self.config,
            RenamerOptions {
                target: Some(target.to_path_buf()),
                format: Some(format.to_owned()),
                ..RenamerOptions::default()
            },
            Box::new(fake),
        )
        .expect("renamer")
    }

    fn renamer(&self, format: &str) -> Renamer {
        self.renamer_with(format, &self.target, Fake::new().0)
    }

    /// The source hashed, with the record that the destination is built from.
    fn episode(&self, renamer: &mut Renamer, overrides: impl FnOnce(&mut FileInfo)) -> Episode {
        let mut episode = renamer.hash_only(&self.source).expect("hash");
        episode.data = Some(sample(overrides));
        episode
    }

    fn hash_cache_path(&self) -> PathBuf {
        let names: Vec<PathBuf> = fs::read_dir(&self.config.cache.path)
            .expect("cache dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.to_string_lossy().ends_with(".hashes.toml"))
            .collect();
        assert_eq!(names.len(), 1);
        names[0].clone()
    }

    fn hash_cache_keys(&self) -> Vec<String> {
        let text = fs::read_to_string(self.hash_cache_path()).expect("read hash cache");
        let table: toml::Table = toml::from_str(&text).expect("parse hash cache");
        table.keys().cloned().collect()
    }
}

fn names(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect()
}

struct CwdGuard(PathBuf);

impl CwdGuard {
    fn enter(dir: &Path) -> Self {
        let old = env::current_dir().expect("cwd");
        env::set_current_dir(dir).expect("chdir");
        Self(old)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        env::set_current_dir(&self.0).expect("restore cwd");
    }
}

#[test]
fn new_validates_its_config() {
    let fx = Fx::new();
    assert!(Renamer::new(&fx.config, RenamerOptions::default()).is_ok());

    let message = Renamer::new(&Config::default(), RenamerOptions::default())
        .unwrap_err()
        .to_string();
    assert!(message.contains("udp_client"), "{message}");

    for missing in ["name", "username", "password"] {
        let mut config = fx.config.clone();
        match missing {
            "name" => config.anidb.udp_client.name = None,
            "username" => config.anidb.udp_client.username = None,
            _ => config.anidb.udp_client.password = None,
        }
        assert!(matches!(
            Renamer::new(&config, RenamerOptions::default()),
            Err(RenamerError::Config)
        ));
    }

    let bad = Renamer::with_client(
        &fx.config,
        RenamerOptions {
            format: Some("{nope}".to_owned()),
            ..RenamerOptions::default()
        },
        Box::new(Fake::new().0),
    );
    assert!(matches!(bad, Err(RenamerError::Format(_))));
}

#[test]
fn identify_hashes_and_caches_without_the_network() {
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);

    let first = renamer.hash_only(&fx.source).expect("first");
    assert_eq!(first.path, fx.source);
    assert_eq!(first.ed2k.hash, ABC_ED2K);
    assert_eq!(first.ed2k.size, 3);
    assert_eq!(
        first.ed2k.link,
        format!("ed2k://|file|source.mkv|3|{ABC_ED2K}|/")
    );
    assert!(first.data.is_none());

    let second = renamer.hash_only(&fx.source).expect("second");
    assert_eq!(second.ed2k.created_at, first.ed2k.created_at);

    let missing = renamer.hash_only(Path::new("/nonexistent/x.mkv"));
    assert!(missing.unwrap_err().to_string().contains("does not exist"));
    let dir = renamer.hash_only(&fx.target);
    assert!(dir.unwrap_err().to_string().contains("must be a file"));
}

#[test]
fn a_changed_size_or_rehash_hashes_again() {
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    let first = renamer.hash_only(&fx.source).expect("first");

    fs::write(&fx.source, "abcd").expect("grow");
    let grown = renamer.hash_only(&fx.source).expect("grown");
    assert_eq!(grown.ed2k.size, 4);
    assert_ne!(grown.ed2k.hash, first.ed2k.hash);

    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut again = Renamer::with_client(
        &fx.config,
        RenamerOptions {
            rehash: true,
            ..RenamerOptions::default()
        },
        Box::new(Fake::new().0),
    )
    .expect("renamer");
    let rehashed = again.hash_only(&fx.source).expect("rehashed");
    assert!(rehashed.ed2k.created_at > grown.ed2k.created_at);
}

#[test]
#[serial]
fn stores_absolute_hash_keys_for_a_relative_target() {
    let fx = Fx::new();
    let _cwd = CwdGuard::enter(fx.target.parent().expect("parent"));
    let mut renamer = fx.renamer_with(
        "{anime_name_romaji}/{episode:number}.{filetype}",
        Path::new("target"),
        Fake::new().0,
    );
    let mut episode = fx.episode(&mut renamer, |_| {});
    let created_at = episode.ed2k.created_at;

    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    assert_eq!(episode.action, Some(Outcome::Move));
    let destination = fx.destination();
    assert_eq!(
        episode.destination_path.as_deref(),
        Some(destination.as_path())
    );
    assert_eq!(fx.hash_cache_keys(), [destination.to_string_lossy()]);

    // a new renamer finds the entry by its absolute path, without rehashing
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut again = fx.renamer(FORMAT);
    let cached = again.hash_only(&destination).expect("cached");
    assert_eq!(cached.ed2k.created_at, created_at);
}

#[test]
fn the_hash_cache_is_rekeyed_after_a_move_and_kept_after_a_copy() {
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Copy, false)
        .expect("copy");
    let mut keys = fx.hash_cache_keys();
    keys.sort();
    assert_eq!(keys.len(), 2);

    // each key keeps a record of its own: the link names that file, so the
    // untouched source is not given the name of its copy
    let table: toml::Table =
        toml::from_str(&fs::read_to_string(fx.hash_cache_path()).expect("read")).expect("parse");
    for key in &keys {
        let file_name = Path::new(key)
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .into_owned();
        let link = table[key.as_str()]["link"].as_str().expect("link");
        assert!(
            link.contains(&format!("|file|{file_name}|")),
            "{key}: {link}"
        );
    }

    // the same file moved: the old key goes, the link names the destination
    fs::remove_file(fx.destination()).expect("remove copy");
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(fx.hash_cache_keys(), [fx.destination().to_string_lossy()]);
    assert_eq!(
        episode.ed2k.link,
        format!("ed2k://|file|13.mkv|3|{ABC_ED2K}|/")
    );
    let text = fs::read_to_string(fx.hash_cache_path()).expect("read");
    assert!(text.contains("13.mkv"));
}

#[test]
#[serial]
fn drops_relative_keys_from_an_old_hash_cache() {
    let fx = Fx::new();
    // seed the cache through a first renamer, so the file name is right
    let mut renamer = fx.renamer(FORMAT);
    let episode = renamer.hash_only(&fx.source).expect("hash");
    let path = fx.hash_cache_path();
    let mut table: toml::Table =
        toml::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
    let entry = table[&episode.path.to_string_lossy().into_owned()].clone();
    // "source.mkv" exists relative to the cwd, so only its relative form can
    // get it purged
    table.insert("source.mkv".to_owned(), entry);
    fs::write(&path, toml::to_string(&table).expect("ser")).expect("write");

    let _cwd = CwdGuard::enter(fx.source.parent().expect("parent"));
    let _renamer = fx.renamer(FORMAT);
    assert_eq!(fx.hash_cache_keys(), [fx.source.to_string_lossy()]);
}

#[test]
fn purges_missing_and_expired_hash_entries() {
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    renamer.hash_only(&fx.source).expect("hash");
    let path = fx.hash_cache_path();
    let mut table: toml::Table =
        toml::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
    let entry = table[&fx.source.to_string_lossy().into_owned()].clone();
    table.insert("/does/not/exist.mkv".to_owned(), entry);
    fs::write(&path, toml::to_string(&table).expect("ser")).expect("write");
    let _ = fx.renamer(FORMAT);
    assert_eq!(fx.hash_cache_keys(), [fx.source.to_string_lossy()]);

    // hash_age is days: a created_at far in the past expires
    let mut table: toml::Table =
        toml::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
    table[&fx.source.to_string_lossy().into_owned()]["createdAt"] = toml::Value::Integer(1_000);
    fs::write(&path, toml::to_string(&table).expect("ser")).expect("write");
    let _ = fx.renamer(FORMAT);
    assert_eq!(fx.hash_cache_keys().len(), 0);
}

#[test]
fn rename_moves_copies_links_and_skips() {
    // without data nothing happens
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    let mut bare = renamer.hash_only(&fx.source).expect("hash");
    renamer
        .rename(&mut bare, Action::Move, false)
        .expect("bare");
    assert_eq!(bare.action, None);
    assert!(fx.source.exists());

    // a dry run changes nothing
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, true)
        .expect("dry run");
    assert_eq!(episode.action, Some(Outcome::Move));
    assert_eq!(episode.destination_path, Some(fx.destination()));
    assert!(fx.source.exists());
    assert_eq!(names(&fx.target).len(), 0);

    // move, with the destination directory created as 0o750
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.action, Some(Outcome::Move));
    assert!(!fx.source.exists());
    assert_eq!(fs::read_to_string(fx.destination()).expect("read"), "abc");
    let mode = fs::metadata(fx.destination().parent().expect("parent"))
        .expect("meta")
        .permissions()
        .mode();
    assert_eq!(mode & 0o700, 0o700);
    assert_eq!(mode & 0o007, 0);

    // copy
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Copy, false)
        .expect("copy");
    assert_eq!(episode.action, Some(Outcome::Copy));
    assert!(fx.source.exists());
    assert!(fx.destination().exists());

    // symlink
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Symlink, false)
        .expect("symlink");
    assert_eq!(episode.action, Some(Outcome::Symlink));
    assert!(fs::symlink_metadata(&fx.source)
        .expect("meta")
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_link(&fx.source).expect("link"), fx.destination());

    // a file already at the destination is uptodate
    let mut fx = Fx::new();
    fs::create_dir_all(fx.destination().parent().expect("parent")).expect("dir");
    fs::rename(&fx.source, fx.destination()).expect("place");
    fx.source = fx.destination();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("uptodate");
    assert_eq!(episode.action, Some(Outcome::UpToDate));

    // an existing destination is kept, unless overwrite is set
    let fx = Fx::new();
    fs::create_dir_all(fx.destination().parent().expect("parent")).expect("dir");
    fs::write(fx.destination(), "existing").expect("existing");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("kept");
    assert_eq!(episode.action, None);
    assert!(fx.source.exists());
    assert_eq!(
        fs::read_to_string(fx.destination()).expect("read"),
        "existing"
    );
    renamer.set_overwrite(true);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("overwrite");
    assert_eq!(fs::read_to_string(fx.destination()).expect("read"), "abc");
    assert!(!fx.source.exists());
}

#[test]
fn rename_handles_moves_across_devices() {
    // a move across devices falls back to copy and delete
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    renamer.rename_fn = |_, _| Err(io::Error::from(io::ErrorKind::CrossesDevices));
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.action, Some(Outcome::Move));
    assert!(!fx.source.exists());
    assert!(fx.destination().exists());

    // a dry run does not copy
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    renamer.rename_fn = |_, _| Err(io::Error::from(io::ErrorKind::CrossesDevices));
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, true)
        .expect("dry run");
    assert!(fx.source.exists());
    assert_eq!(names(&fx.target).len(), 0);

    // other errors are rethrown
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    renamer.rename_fn = |_, _| Err(io::Error::from(io::ErrorKind::PermissionDenied));
    let mut episode = fx.episode(&mut renamer, |_| {});
    let error = renamer
        .rename(&mut episode, Action::Move, false)
        .unwrap_err();
    assert!(matches!(error, RenamerError::Io { .. }), "{error}");
    assert!(fx.source.exists());
}

#[test]
fn rename_keeps_destinations_inside_the_target() {
    // a .. tag value, both in the middle and as the basename
    for format in [
        "{anime_name_romaji}/{anime_name_romaji}",
        "a/{anime_name_romaji}",
    ] {
        let fx = Fx::new();
        let mut renamer = fx.renamer(format);
        let mut episode = fx.episode(&mut renamer, |i| i.anime_name_romaji = "..".to_owned());
        let error = renamer
            .rename(&mut episode, Action::Move, false)
            .unwrap_err();
        assert!(error.to_string().contains("outside of"), "{error}");
        assert!(fx.source.exists());
    }

    // a template that climbs out is neutralised by the formatter
    let fx = Fx::new();
    let mut renamer = fx.renamer("../escaped.{filetype}");
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    assert!(fx.target.join("escaped.mkv").exists());
    assert!(!fx
        .target
        .parent()
        .expect("parent")
        .join("escaped.mkv")
        .exists());

    // the target itself may be written with dots
    let fx = Fx::new();
    let target = fx.target.join("nested").join("..").join("renamed");
    let mut renamer = fx.renamer_with(FORMAT, &target, Fake::new().0);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    assert_eq!(episode.action, Some(Outcome::Move));
    assert!(fx.target.join("renamed/Test Anime/13.mkv").exists());
}

#[test]
fn rename_shortens_an_episode_name_that_is_too_long() {
    let fx = Fx::new();
    let mut renamer = fx.renamer("{anime_name_romaji}/{episode_name}.{filetype}");
    let mut episode = fx.episode(&mut renamer, |i| i.episode_name = "a".repeat(300));
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    let destination = episode.destination_path.expect("destination");
    let name = destination.file_name().expect("name").to_string_lossy();
    assert_eq!(episode.action, Some(Outcome::Move));
    assert!(name.ends_with("~.mkv"), "{name}");
    assert_eq!(name.len(), NAME_MAX);
    assert!(!fx.source.exists());
    assert!(destination.exists());
}

#[test]
fn rename_swaps_a_long_anime_name_for_its_short_name() {
    let format = "{anime_name_romaji}/{anime_name_romaji} - {episode_name}.{filetype}";
    let fx = Fx::new();
    let mut renamer = fx.renamer(format);
    let mut episode = fx.episode(&mut renamer, |i| {
        i.anime_name_romaji = "a".repeat(300);
        i.anime_name_short = vec![String::new(), "Shorty".into(), "Longer one".into()];
        i.episode_name = "Name".to_owned();
    });
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    assert_eq!(
        episode.destination_path,
        Some(fx.target.join("Shorty").join("Shorty - Name.mkv"))
    );

    // both long: short name and a cut episode name at once
    let fx = Fx::new();
    let mut renamer = fx.renamer(format);
    let mut episode = fx.episode(&mut renamer, |i| {
        i.anime_name_romaji = "a".repeat(300);
        i.anime_name_short = vec!["Shorty".into()];
        i.episode_name = "b".repeat(300);
    });
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    let destination = episode.destination_path.expect("destination");
    let name = destination.file_name().expect("name").to_string_lossy();
    assert!(name.starts_with("Shorty - b"), "{name}");
    assert!(name.ends_with("~.mkv"), "{name}");
}

#[test]
fn rename_falls_back_to_episode_n() {
    // an episode name of 255 bytes cannot be cut to fit with a long anime name
    let fx = Fx::new();
    let mut renamer = fx.renamer("{anime_name_romaji} - {episode_name}.{filetype}");
    let mut episode = fx.episode(&mut renamer, |i| {
        i.anime_name_romaji = "a".repeat(235);
        i.anime_name_short = Vec::new();
        i.episode = "013".to_owned();
        i.episode_name = "b".repeat(100);
    });
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    let destination = episode.destination_path.expect("destination");
    assert_eq!(
        destination.file_name().expect("name").to_string_lossy(),
        format!("{} - Episode 13.mkv", "a".repeat(235))
    );
}

#[test]
fn a_dry_run_also_shortens_too_long_names() {
    let fx = Fx::new();
    let mut renamer = fx.renamer("{anime_name_romaji}/{episode_name}.{filetype}");
    let mut episode = fx.episode(&mut renamer, |i| i.episode_name = "a".repeat(300));
    renamer
        .rename(&mut episode, Action::Move, true)
        .expect("dry run");
    let destination = episode.destination_path.expect("destination");
    assert!(destination.to_string_lossy().ends_with("~.mkv"));
    assert_eq!(names(&fx.target).len(), 0);
}

#[test]
fn shorten_helpers_truncate_as_expected() {
    assert_eq!(truncate_name("short", 10), "short");
    assert_eq!(truncate_name("abc def ghi", 4), "abc~");
    assert_eq!(truncate_name("日本語です", 3), "日本語~");
    let template = Template::parse("{episode_name}").expect("template");
    let data = sample(|i| i.episode = "0013-0014".to_owned());
    let result = shorten_names(&data, &template, 1).expect("shorten");
    assert_eq!(result.episode_name, "Episode 13-14");
    let data = sample(|i| i.episode = "S01".to_owned());
    let result = shorten_names(&data, &template, 1).expect("shorten");
    assert_eq!(result.episode_name, "Episode S01");
}

#[test]
fn episode_fallback_names_drop_the_leading_zeros_of_each_number() {
    let template = Template::parse("{episode_name}").expect("template");
    for (episode, expected) in [
        ("013", "Episode 13"),
        ("1", "Episode 1"),
        ("0", "Episode 0"),
        ("000", "Episode 0"),
        ("10", "Episode 10"),
        ("0100", "Episode 100"),
        ("01-02", "Episode 1-2"),
        ("C02", "Episode C02"),
        ("01 02", "Episode 1 2"),
    ] {
        let data = sample(|i| i.episode = episode.to_owned());
        let result = shorten_names(&data, &template, 1).expect("shorten");
        assert_eq!(result.episode_name, expected, "{episode}");
        assert_eq!(result.episode_name_romaji, expected, "{episode}");
    }
}

#[test]
fn identify_returns_no_data_for_a_hash_unknown_to_anidb() {
    let fx = Fx::new();
    let (fake, calls) = Fake::new();
    let mut renamer = fx.renamer_with(FORMAT, &fx.target, fake);
    let episode = renamer.identify(&fx.source).expect("identify");
    assert!(episode.data.is_none());
    assert_eq!(episode.ed2k.hash, ABC_ED2K);
    assert_eq!(calls.borrow().file, 1);
    assert_eq!(calls.borrow().disconnects, 0);
    let anidb = fx.config.cache.path.join("anidb");
    let written = if anidb.exists() {
        names(&anidb)
    } else {
        Vec::new()
    };
    assert!(written.is_empty(), "{written:?}");
}

#[test]
fn identify_disconnects_and_fails_on_other_anidb_errors() {
    let fx = Fx::new();
    let (mut fake, calls) = Fake::new();
    fake.file = Box::new(|| Err(reply_error(ResponseCode::Banned)));
    let mut renamer = fx.renamer_with(FORMAT, &fx.target, fake);
    let error = renamer.identify(&fx.source).unwrap_err();
    assert!(matches!(error, RenamerError::Anidb(_)), "{error}");
    assert_eq!(calls.borrow().disconnects, 1);
}

#[test]
fn identify_fills_missing_hashes_and_caches_the_record() {
    let fx = Fx::new();
    let (mut fake, calls) = Fake::new();
    fake.file = Box::new(|| {
        Ok(sample(|i| {
            i.crc32.clear();
            i.md5.clear();
            i.sha1.clear();
        }))
    });
    let mut renamer = fx.renamer_with(FORMAT, &fx.target, fake);
    let episode = renamer.identify(&fx.source).expect("identify");
    let data = episode.data.expect("data");
    assert_eq!(data.crc32, "352441c2");
    assert_eq!(data.md5, "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(data.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");

    // the second identify comes from the fid cache, not from AniDB
    let again = renamer.identify(&fx.source).expect("again");
    assert_eq!(again.data.as_ref(), Some(&data));
    assert_eq!(calls.borrow().file, 1);

    // refresh asks again
    renamer
        .identify_with(&fx.source, true, false)
        .expect("refresh");
    assert_eq!(calls.borrow().file, 2);
    let fid = fx
        .config
        .cache
        .path
        .join("anidb")
        .join(format!("{ABC_ED2K}.fid.toml"));
    assert!(fid.exists());
}

#[test]
fn set_mylist_state_adds_edits_and_skips() {
    let fx = Fx::new();
    let episode = fx.renamer(FORMAT).hash_only(&fx.source).expect("hash");

    // no entry: add
    let (fake, calls) = Fake::new();
    let mut renamer = fx.renamer_with(FORMAT, &fx.target, fake);
    assert!(renamer
        .set_mylist_state(&episode, MylistState::InternalStorage)
        .expect("add"));
    assert_eq!(
        calls.borrow().adds,
        [MylistAdd {
            edit: false,
            state: Some(MylistState::InternalStorage)
        }]
    );

    // another state: edit
    let entry = |state| MylistEntry {
        lid: 1,
        fid: 2,
        eid: 3,
        aid: 4,
        gid: 5,
        date: 0,
        state,
        viewdate: 0,
        storage: String::new(),
        source: String::new(),
        other: String::new(),
        filestate: None,
    };
    let (mut fake, calls) = Fake::new();
    fake.mylist = Box::new(move || Ok(entry(Some(MylistState::Unknown))));
    fake.add = Box::new(|_| Ok(ResponseCode::MylistEntryEdited));
    let mut renamer = fx.renamer_with(FORMAT, &fx.target, fake);
    assert!(renamer
        .set_mylist_state(&episode, MylistState::InternalStorage)
        .expect("edit"));
    assert!(calls.borrow().adds[0].edit);

    // already the state: nothing to do
    let (mut fake, calls) = Fake::new();
    fake.mylist = Box::new(move || Ok(entry(Some(MylistState::InternalStorage))));
    let mut renamer = fx.renamer_with(FORMAT, &fx.target, fake);
    assert!(renamer
        .set_mylist_state(&episode, MylistState::InternalStorage)
        .expect("same"));
    assert_eq!(calls.borrow().adds.len(), 0);

    // an unexpected add code is false
    let (mut fake, _calls) = Fake::new();
    fake.add = Box::new(|_| Ok(ResponseCode::Unknown(999)));
    let mut renamer = fx.renamer_with(FORMAT, &fx.target, fake);
    assert!(!renamer
        .set_mylist_state(&episode, MylistState::Deleted)
        .expect("false"));

    // other errors disconnect and fail
    let (mut fake, calls) = Fake::new();
    fake.mylist = Box::new(|| Err(reply_error(ResponseCode::Banned)));
    let mut renamer = fx.renamer_with(FORMAT, &fx.target, fake);
    assert!(renamer
        .set_mylist_state(&episode, MylistState::Deleted)
        .is_err());
    assert_eq!(calls.borrow().disconnects, 1);
    let _ = &fx.dir;
}

#[test]
fn file_info_maps_a_file_reply() {
    let mut file = std::collections::BTreeMap::new();
    file.insert(FileField::Aid, Value::Int(10_914));
    file.insert(FileField::State, Value::Int(0x1005 | 0x80));
    file.insert(FileField::Size, Value::Int(447_945_964));
    file.insert(FileField::Crc32, Value::Str("438a7b93".into()));
    file.insert(
        FileField::DubLanguage,
        Value::Strs(vec!["japanese".into(), "english".into()]),
    );
    file.insert(FileField::AudioBitrates, Value::Ints(vec![271, 128]));
    file.insert(FileField::VideoBitrate, Value::Int(0));
    let mut anime = std::collections::BTreeMap::new();
    anime.insert(AnimeField::Year, Value::Str("2015-2016".into()));
    anime.insert(AnimeField::Epno, Value::Str("S1".into()));
    anime.insert(AnimeField::ShortNameList, Value::Strs(vec!["T7".into()]));
    let info = file_info_from_reply(&FileReply {
        fid: 1_628_637,
        file,
        anime,
    });
    assert_eq!(info.fid, 1_628_637);
    assert_eq!(info.aid, 10_914);
    assert_eq!(info.size, 447_945_964);
    assert_eq!(info.version, "v2");
    assert_eq!(info.censored, "cen");
    assert_eq!(info.lang_dub, "japanese'english");
    assert_eq!(info.audio_bitrate, "271'128");
    assert_eq!(info.video_bitrate, "");
    assert_eq!(info.anime_year, [2015, 2016]);
    assert_eq!(info.episode, "S1");
    assert_eq!(info.anime_name_short, ["T7"]);

    let mut state = std::collections::BTreeMap::new();
    state.insert(FileField::State, Value::Int(0x08 | 0x10 | 0x40 | 0x80));
    let info = file_info_from_reply(&FileReply {
        fid: 1,
        file: state,
        anime: std::collections::BTreeMap::new(),
    });
    assert_eq!(info.version, "v4");
    assert_eq!(info.censored, "unc");
}

#[test]
fn the_masks_match_the_reference_hex() {
    assert_eq!(file_fields().to_hex(), "79f8fff100");
    assert_eq!(anime_fields().to_hex(), "f2fcf0c0");
}

impl Fx {
    fn nfo_source(&self) -> PathBuf {
        self.source.with_extension("nfo")
    }

    fn nfo_destination(&self) -> PathBuf {
        self.destination().with_extension("nfo")
    }

    fn with_nfo(self) -> Self {
        fs::write(self.nfo_source(), "<episodedetails/>").expect("nfo");
        self
    }
}

#[test]
fn the_sidecar_nfo_follows_a_move() {
    let fx = Fx::new().with_nfo();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert!(!fx.nfo_source().exists());
    assert_eq!(
        fs::read_to_string(fx.nfo_destination()).expect("nfo"),
        "<episodedetails/>"
    );
    assert_eq!(
        episode.sidecars,
        vec![SidecarResult {
            source: fx.nfo_source(),
            destination: fx.nfo_destination(),
            action: Outcome::Move,
            error: None,
        }]
    );
}

#[test]
fn the_sidecar_nfo_follows_a_copy_and_a_symlink() {
    let fx = Fx::new().with_nfo();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Copy, false)
        .expect("copy");
    assert!(fx.nfo_source().exists());
    assert!(fx.nfo_destination().exists());
    assert_eq!(episode.sidecars[0].action, Outcome::Copy);

    let fx = Fx::new().with_nfo();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Symlink, false)
        .expect("symlink");
    assert_eq!(
        fs::read_link(fx.nfo_source()).expect("link"),
        fx.nfo_destination()
    );
    assert_eq!(
        fs::read_to_string(fx.nfo_destination()).expect("nfo"),
        "<episodedetails/>"
    );
    assert_eq!(episode.sidecars[0].action, Outcome::Symlink);
}

#[test]
fn a_dry_run_reports_the_sidecar_and_touches_nothing() {
    let fx = Fx::new().with_nfo();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, true)
        .expect("dry run");
    assert_eq!(episode.sidecars.len(), 1);
    assert_eq!(episode.sidecars[0].destination, fx.nfo_destination());
    assert_eq!(episode.sidecars[0].error, None);
    assert!(fx.nfo_source().exists());
    assert_eq!(names(&fx.target).len(), 0);
}

#[test]
fn no_sidecar_means_nothing_to_report() {
    let fx = Fx::new();
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars.len(), 0);
    assert_eq!(names(fx.destination().parent().expect("dir")).len(), 1);
}

#[test]
fn the_sidecar_extension_is_matched_case_insensitively() {
    let fx = Fx::new();
    fs::write(fx.source.with_extension("NFO"), "x").expect("nfo");
    // not a sidecar: another stem
    fs::write(fx.source.with_file_name("source 2.nfo"), "y").expect("other");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars.len(), 1);
    assert!(fx.destination().with_extension("NFO").exists());
    assert!(fx.source.with_file_name("source 2.nfo").exists());
}

#[test]
fn an_existing_sidecar_destination_is_kept_unless_overwrite_is_set() {
    let fx = Fx::new().with_nfo();
    fs::create_dir_all(fx.destination().parent().expect("parent")).expect("dir");
    fs::write(fx.nfo_destination(), "existing").expect("existing");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars.len(), 0);
    assert!(fx.nfo_source().exists());
    assert_eq!(
        fs::read_to_string(fx.nfo_destination()).expect("nfo"),
        "existing"
    );

    // the video was moved, so restore it for the second run
    fs::rename(fx.destination(), &fx.source).expect("restore");
    renamer.set_overwrite(true);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("overwrite");
    assert!(!fx.nfo_source().exists());
    assert_eq!(
        fs::read_to_string(fx.nfo_destination()).expect("nfo"),
        "<episodedetails/>"
    );
}

#[test]
fn the_sidecar_follows_the_shortened_name() {
    let fx = Fx::new().with_nfo();
    let mut renamer = fx.renamer("{anime_name_romaji}/{episode_name}.{filetype}");
    let mut episode = fx.episode(&mut renamer, |i| i.episode_name = "a".repeat(300));
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    let destination = episode.destination_path.clone().expect("destination");
    let nfo = destination.with_extension("nfo");
    assert!(nfo.exists(), "{}", nfo.display());
    assert!(nfo
        .file_name()
        .expect("name")
        .to_string_lossy()
        .ends_with("~.nfo"));
    assert_eq!(episode.sidecars[0].destination, nfo);
    assert!(!fx.nfo_source().exists());
}

#[test]
fn a_read_only_sidecar_is_still_moved() {
    for mode in [0o400, 0o500] {
        let fx = Fx::new().with_nfo();
        fs::set_permissions(fx.nfo_source(), fs::Permissions::from_mode(mode)).expect("chmod");
        let mut renamer = fx.renamer(FORMAT);
        let mut episode = fx.episode(&mut renamer, |_| {});
        renamer
            .rename(&mut episode, Action::Move, false)
            .expect("move");
        assert_eq!(episode.sidecars[0].error, None);
        assert!(!fx.nfo_source().exists());
        let moved = fs::metadata(fx.nfo_destination()).expect("meta");
        assert_eq!(moved.permissions().mode() & 0o777, mode);
    }
}

#[test]
fn the_sidecar_crosses_devices_like_the_video() {
    let fx = Fx::new().with_nfo();
    fs::set_permissions(fx.nfo_source(), fs::Permissions::from_mode(0o400)).expect("chmod");
    let mut renamer = fx.renamer(FORMAT);
    renamer.rename_fn = |_, _| Err(io::Error::from(io::ErrorKind::CrossesDevices));
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars[0].error, None);
    assert!(!fx.nfo_source().exists());
    assert_eq!(
        fs::read_to_string(fx.nfo_destination()).expect("nfo"),
        "<episodedetails/>"
    );
}

#[test]
fn a_sidecar_failure_is_reported_and_keeps_the_episode_renamed() {
    let fx = Fx::new().with_nfo();
    let mut renamer = fx.renamer(FORMAT);
    // the video moves, the sidecar is refused
    renamer.rename_fn = |from, to| {
        if from.extension().is_some_and(|e| e == "nfo") {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        } else {
            fs::rename(from, to)
        }
    };
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("the episode is renamed");
    assert!(fx.destination().exists());
    assert!(!fx.source.exists());
    assert!(fx.nfo_source().exists());
    assert_eq!(episode.action, Some(Outcome::Move));
    assert_eq!(episode.sidecars.len(), 1);
    assert!(episode.sidecars[0].error.is_some());
    // the hash cache follows the episode regardless
    assert!(fx
        .hash_cache_keys()
        .contains(&fx.destination().to_string_lossy().into_owned()));
}

#[test]
fn an_up_to_date_episode_leaves_its_sidecar_alone() {
    let mut fx = Fx::new();
    fs::create_dir_all(fx.destination().parent().expect("parent")).expect("dir");
    fs::rename(&fx.source, fx.destination()).expect("place");
    fx.source = fx.destination();
    fs::write(fx.nfo_source(), "x").expect("nfo");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("uptodate");
    assert_eq!(episode.action, Some(Outcome::UpToDate));
    assert_eq!(episode.sidecars.len(), 0);
    assert_eq!(fs::read_to_string(fx.nfo_source()).expect("nfo"), "x");
}

/// Writes one sidecar per name next to the source episode, each holding its name.
fn write_sidecars(fx: &Fx, sidecars: &[&str]) {
    for name in sidecars {
        fs::write(fx.source.with_file_name(name), name).expect("sidecar");
    }
}

/// Renames the source episode, so that its sidecars can have another stem.
fn rename_source(fx: &mut Fx, name: &str) {
    let renamed = fx.source.with_file_name(name);
    fs::rename(&fx.source, &renamed).expect("rename source");
    fx.source = renamed;
}

/// The file `name` in the directory the episode goes to.
fn in_destination_dir(fx: &Fx, name: &str) -> PathBuf {
    fx.destination().with_file_name(name)
}

#[test]
fn a_plain_subtitle_follows_the_episode() {
    let fx = Fx::new();
    write_sidecars(&fx, &["source.srt", "source.ass"]);
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    let mut found = names(&fx.target.join("Test Anime"));
    found.sort();
    assert_eq!(found, ["13.ass", "13.mkv", "13.srt"]);
    assert_eq!(
        fs::read_to_string(in_destination_dir(&fx, "13.srt")).expect("srt"),
        "source.srt"
    );
    assert!(!fx.source.with_file_name("source.srt").exists());
    assert_eq!(episode.sidecars.len(), 2);
    assert!(episode.sidecars.iter().all(|s| s.action == Outcome::Move));
}

#[test]
fn subtitle_tags_between_the_stem_and_the_extension_are_kept() {
    let fx = Fx::new();
    write_sidecars(
        &fx,
        &[
            "source.en.srt",
            "source.eng.forced.ass",
            "source.pt-BR.sdh.srt",
        ],
    );
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    for name in ["13.en.srt", "13.eng.forced.ass", "13.pt-BR.sdh.srt"] {
        assert!(in_destination_dir(&fx, name).exists(), "{name}");
    }
    assert_eq!(episode.sidecars.len(), 3);
    assert_eq!(names(&fx.target.join("Test Anime")).len(), 4);
}

#[test]
fn every_sidecar_kind_moves_with_the_episode() {
    let fx = Fx::new();
    let sidecars = [
        "source.nfo",
        "source.sub",
        "source.idx",
        "source.de.srt",
        "source.sup",
        "source.vtt",
        "source.ssa",
        "source.mks",
    ];
    write_sidecars(&fx, &sidecars);
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars.len(), sidecars.len());
    for sidecar in &sidecars {
        assert!(!fx.source.with_file_name(sidecar).exists(), "{sidecar}");
    }
    for name in [
        "13.nfo",
        "13.sub",
        "13.idx",
        "13.de.srt",
        "13.sup",
        "13.vtt",
        "13.ssa",
        "13.mks",
    ] {
        assert!(in_destination_dir(&fx, name).exists(), "{name}");
    }
}

#[test]
fn a_sidecar_stem_must_be_followed_by_a_dot() {
    let mut fx = Fx::new();
    rename_source(&mut fx, "Show - 1.mkv");
    write_sidecars(
        &fx,
        &["Show - 1.srt", "Show - 10.srt", "Show - 1 - Name.srt"],
    );
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars.len(), 1);
    assert!(in_destination_dir(&fx, "13.srt").exists());
    assert!(fx.source.with_file_name("Show - 10.srt").exists());
    assert!(fx.source.with_file_name("Show - 1 - Name.srt").exists());
}

#[test]
fn a_longer_episode_number_keeps_its_own_sidecars() {
    let mut fx = Fx::new();
    rename_source(&mut fx, "Show - 1.mkv");
    write_sidecars(&fx, &["Show - 1.srt", "Show - 1.5.srt"]);
    fs::write(fx.source.with_file_name("Show - 1.5.mkv"), "x").expect("episode");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars.len(), 1);
    assert!(in_destination_dir(&fx, "13.srt").exists());
    assert!(fx.source.with_file_name("Show - 1.5.srt").exists());
}

#[test]
fn subtitle_extensions_match_case_insensitively_and_keep_their_spelling() {
    let fx = Fx::new();
    write_sidecars(&fx, &["source.EN.SRT", "source.Ass"]);
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert!(in_destination_dir(&fx, "13.EN.SRT").exists());
    assert!(in_destination_dir(&fx, "13.Ass").exists());
    assert_eq!(episode.sidecars.len(), 2);
}

#[test]
fn subtitles_follow_a_copy_and_a_symlink() {
    let fx = Fx::new();
    write_sidecars(&fx, &["source.en.srt"]);
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Copy, false)
        .expect("copy");
    assert!(fx.source.with_file_name("source.en.srt").exists());
    assert!(in_destination_dir(&fx, "13.en.srt").exists());
    assert_eq!(episode.sidecars[0].action, Outcome::Copy);

    let fx = Fx::new();
    write_sidecars(&fx, &["source.en.srt"]);
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Symlink, false)
        .expect("symlink");
    assert_eq!(
        fs::read_link(fx.source.with_file_name("source.en.srt")).expect("link"),
        in_destination_dir(&fx, "13.en.srt")
    );
    assert_eq!(
        fs::read_to_string(in_destination_dir(&fx, "13.en.srt")).expect("srt"),
        "source.en.srt"
    );
    assert_eq!(episode.sidecars[0].action, Outcome::Symlink);
}

#[test]
fn a_dry_run_reports_subtitles_and_touches_nothing() {
    let fx = Fx::new();
    write_sidecars(&fx, &["source.en.srt", "source.ass"]);
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, true)
        .expect("dry run");
    assert_eq!(episode.sidecars.len(), 2);
    assert!(episode.sidecars.iter().all(|s| s.error.is_none()));
    assert!(episode
        .sidecars
        .iter()
        .any(|s| s.destination == in_destination_dir(&fx, "13.en.srt")));
    assert!(fx.source.with_file_name("source.en.srt").exists());
    assert!(fx.source.with_file_name("source.ass").exists());
    assert_eq!(names(&fx.target).len(), 0);
}

#[test]
fn an_existing_subtitle_destination_is_kept_unless_overwrite_is_set() {
    let fx = Fx::new();
    write_sidecars(&fx, &["source.en.srt"]);
    fs::create_dir_all(fx.destination().parent().expect("parent")).expect("dir");
    fs::write(in_destination_dir(&fx, "13.en.srt"), "existing").expect("existing");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars.len(), 0);
    assert!(fx.source.with_file_name("source.en.srt").exists());
    assert_eq!(
        fs::read_to_string(in_destination_dir(&fx, "13.en.srt")).expect("srt"),
        "existing"
    );

    let fx = Fx::new();
    write_sidecars(&fx, &["source.en.srt"]);
    fs::create_dir_all(fx.destination().parent().expect("parent")).expect("dir");
    fs::write(in_destination_dir(&fx, "13.en.srt"), "existing").expect("existing");
    let mut renamer = fx.renamer(FORMAT);
    renamer.set_overwrite(true);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("overwrite");
    assert!(!fx.source.with_file_name("source.en.srt").exists());
    assert_eq!(
        fs::read_to_string(in_destination_dir(&fx, "13.en.srt")).expect("srt"),
        "source.en.srt"
    );
}

#[test]
fn a_subtitle_crosses_devices_like_the_episode() {
    let fx = Fx::new();
    write_sidecars(&fx, &["source.pt-BR.sdh.srt"]);
    let sidecar = fx.source.with_file_name("source.pt-BR.sdh.srt");
    fs::set_permissions(&sidecar, fs::Permissions::from_mode(0o400)).expect("chmod");
    let mut renamer = fx.renamer(FORMAT);
    renamer.rename_fn = |_, _| Err(io::Error::from(io::ErrorKind::CrossesDevices));
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(episode.sidecars[0].error, None);
    assert!(!sidecar.exists());
    assert_eq!(
        fs::read_to_string(in_destination_dir(&fx, "13.pt-BR.sdh.srt")).expect("srt"),
        "source.pt-BR.sdh.srt"
    );
}

#[test]
fn a_failing_subtitle_keeps_the_episode_renamed_and_the_others_moving() {
    let fx = Fx::new();
    write_sidecars(&fx, &["source.nfo", "source.en.srt"]);
    let mut renamer = fx.renamer(FORMAT);
    renamer.rename_fn = |from, to| {
        if from.extension().is_some_and(|e| e == "srt") {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        } else {
            fs::rename(from, to)
        }
    };
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("the episode is renamed");
    assert!(in_destination_dir(&fx, "13.mkv").exists());
    assert!(!fx.source.exists());
    assert!(fx.source.with_file_name("source.en.srt").exists());
    assert!(!fx.source.with_file_name("source.nfo").exists());
    assert_eq!(episode.sidecars.len(), 2);
    let failed = episode
        .sidecars
        .iter()
        .find(|s| s.source.ends_with("source.en.srt"))
        .expect("the subtitle is reported");
    assert!(failed.error.is_some());
    let moved = episode
        .sidecars
        .iter()
        .find(|s| s.source.ends_with("source.nfo"))
        .expect("the nfo is reported");
    assert_eq!(moved.error, None);
}

#[test]
fn an_episode_is_never_its_own_sidecar() {
    let mut fx = Fx::new();
    rename_source(&mut fx, "source.mks");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Copy, false)
        .expect("copy");
    assert_eq!(episode.sidecars.len(), 0);
    assert!(fx.source.exists());
    assert!(in_destination_dir(&fx, "13.mkv").exists());
}

#[test]
fn a_subtitle_name_too_long_for_the_file_system_cuts_only_its_stem() {
    let fx = Fx::new();
    write_sidecars(&fx, &["source.pt-BR.sdh.srt"]);
    let mut renamer = fx.renamer("{anime_name_romaji}/{episode_name}.{filetype}");
    let mut episode = fx.episode(&mut renamer, |i| i.episode_name = "a".repeat(250));
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("rename");
    let video = episode.destination_path.clone().expect("destination");
    assert_eq!(
        video.file_name().expect("name").to_string_lossy(),
        format!("{}.mkv", "a".repeat(250))
    );
    assert_eq!(episode.sidecars.len(), 1);
    let sidecar = &episode.sidecars[0];
    assert_eq!(sidecar.error, None);
    let name = sidecar
        .destination
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    assert_eq!(name, format!("{}~.pt-BR.sdh.srt", "a".repeat(240)));
    assert!(name.len() <= 255);
    assert!(sidecar.destination.exists());
}

fn set_source_time(fx: &Fx, seconds: u64) -> SystemTime {
    let time = UNIX_EPOCH + Duration::from_secs(seconds);
    set_modified(&fx.source, time).expect("set time");
    time
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path)
        .expect("metadata")
        .modified()
        .expect("mtime")
}

#[test]
fn a_move_on_one_file_system_keeps_the_file_time() {
    let fx = Fx::new().with_nfo();
    let time = set_source_time(&fx, 1_500_000_000);
    set_modified(&fx.nfo_source(), time).expect("nfo time");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert_eq!(mtime(&fx.destination()), time);
    assert_eq!(mtime(&fx.nfo_destination()), time);
}

#[test]
fn a_copy_keeps_the_file_time() {
    let fx = Fx::new().with_nfo();
    let time = set_source_time(&fx, 1_500_000_000);
    let nfo_time = UNIX_EPOCH + Duration::from_secs(1_400_000_000);
    set_modified(&fx.nfo_source(), nfo_time).expect("nfo time");
    let mut renamer = fx.renamer(FORMAT);
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Copy, false)
        .expect("copy");
    assert_eq!(mtime(&fx.destination()), time);
    assert_eq!(mtime(&fx.nfo_destination()), nfo_time);
    assert_eq!(mtime(&fx.source), time);
}

#[test]
fn a_move_across_devices_keeps_the_file_time() {
    let fx = Fx::new().with_nfo();
    let time = set_source_time(&fx, 1_500_000_000);
    let nfo_time = UNIX_EPOCH + Duration::from_secs(1_400_000_000);
    set_modified(&fx.nfo_source(), nfo_time).expect("nfo time");
    let mut renamer = fx.renamer(FORMAT);
    renamer.rename_fn = |_, _| Err(io::Error::from(io::ErrorKind::CrossesDevices));
    let mut episode = fx.episode(&mut renamer, |_| {});
    renamer
        .rename(&mut episode, Action::Move, false)
        .expect("move");
    assert!(!fx.source.exists());
    assert_eq!(mtime(&fx.destination()), time);
    assert_eq!(mtime(&fx.nfo_destination()), nfo_time);
}
