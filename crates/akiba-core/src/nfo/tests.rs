use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

fn id(id: &str, kind: &str, default: bool) -> UniqueId {
    UniqueId {
        id: id.to_string(),
        kind: kind.to_string(),
        default,
    }
}

fn show(ids: Vec<UniqueId>) -> TvShow {
    TvShow {
        unique_ids: ids,
        title: "Test Anime".to_string(),
        ..TvShow::default()
    }
}

fn episode(ids: Vec<UniqueId>) -> Episode {
    Episode {
        unique_ids: ids,
        title: "First".to_string(),
        ..Episode::default()
    }
}

fn episode_file(dir: &Path) -> PathBuf {
    let file = dir.join("Show - 01 - First.mkv");
    fs::write(&file, "").expect("episode file");
    file
}

#[test]
fn tv_show_validates_path_and_data() {
    let dir = tempfile::tempdir().expect("tempdir");
    let err = TvShowNfo::new(show(vec![id("1", "anidb", true)]), "/nonexistent")
        .expect_err("missing dir");
    assert!(matches!(err, NfoError::NotADirectory(_)));
    assert!(err.to_string().contains("is not a directory"));

    let valid = |ids: Vec<UniqueId>, premiered: Option<&str>| {
        let mut tv = show(ids);
        tv.premiered = premiered.map(str::to_string);
        TvShowNfo::new(tv, dir.path()).expect("dir").is_valid()
    };
    assert!(!valid(vec![], None));
    assert!(valid(vec![id("1", "anidb", false)], None));
    assert!(!valid(
        vec![id("1", "anidb", true), id("2", "anilist", true)],
        None
    ));
    assert!(!valid(
        vec![id("1", "anidb", false), id("2", "anilist", false)],
        None
    ));
    assert!(!valid(vec![id("1", "anidb", true)], Some("2020-01")));
    assert!(!valid(vec![id("1", "anidb", true)], Some("2020-13-45")));
    assert!(valid(vec![id("1", "anidb", true)], Some("2020-01-02")));
    assert!(valid(vec![id("1", "anidb", true)], Some("")));

    // a single unique id becomes the default
    let mut nfo = TvShowNfo::new(show(vec![id("1", "anidb", false)]), dir.path()).expect("dir");
    assert!(nfo.is_valid());
    assert!(nfo.tv_show.unique_ids[0].default);
}

#[test]
fn tv_show_write_rejects_invalid_data() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut nfo = TvShowNfo::new(show(vec![]), dir.path()).expect("dir");
    let err = nfo.write(false, None).expect_err("invalid");
    assert!(matches!(err, NfoError::Invalid(_)));
    assert!(!dir.path().join(TV_SHOW_FILE).exists());
}

#[test]
fn tv_show_write_writes_every_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut tv = show(vec![
        id("1", "anidb", true),
        id("2", "anilist", false),
        id("3", "tmdb", false),
    ]);
    tv.original_title = Some("テスト".to_string());
    tv.sort_title = Some("Sort".to_string());
    tv.premiered = Some("2020-01-02".to_string());
    tv.plot = Some("A plot".to_string());
    tv.mpaa = Some("G".to_string());
    tv.tags = vec!["Action".to_string()];
    tv.studios = vec!["Studio".to_string()];
    tv.actors = vec![Actor {
        name: "Name".to_string(),
        role: "Role".to_string(),
        order: Some(0),
    }];
    let mut nfo = TvShowNfo::new(tv, dir.path()).expect("dir");
    nfo.write(false, None).expect("write");

    let xml = fs::read(dir.path().join(TV_SHOW_FILE)).expect("nfo");
    let text = String::from_utf8(xml.clone()).expect("utf-8");
    assert!(text.starts_with(r#"<?xml version="1.0" encoding="utf-8"?>"#));
    for expected in [
        "<title>Test Anime</title>",
        "<originaltitle>テスト</originaltitle>",
        "<sorttitle>Sort</sorttitle>",
        r#"type="AniDB" default="true""#,
        r#"type="AniList" default="false""#,
        r#"type="tmdb" default="false""#,
        "<premiered>2020-01-02</premiered>",
        "<plot>A plot</plot>",
        "<mpaa>G</mpaa>",
        "<tag>Action</tag>",
        "<studio>Studio</studio>",
        "<name>Name</name>",
        "<order>0</order>",
    ] {
        assert!(text.contains(expected), "missing {expected} in {text}");
    }
}

#[test]
fn tv_show_write_keeps_existing_nfo_unless_overwrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let nfo_path = dir.path().join(TV_SHOW_FILE);
    fs::write(&nfo_path, "old").expect("seed");

    let mut nfo = TvShowNfo::new(show(vec![id("1", "anidb", true)]), dir.path()).expect("dir");
    nfo.write(false, None).expect("write");
    assert_eq!(fs::read_to_string(&nfo_path).expect("read"), "old");

    nfo.write(true, None).expect("overwrite");
    let minimal = fs::read_to_string(&nfo_path).expect("read");
    assert!(minimal.contains("<tvshow>"));
    assert!(minimal.contains("<title>Test Anime</title>"));
}

#[test]
fn tv_show_escapes_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut tv = show(vec![id("1", "anidb", true)]);
    tv.title = "A <b> & 'c' \"d\"".to_string();
    TvShowNfo::new(tv, dir.path())
        .expect("dir")
        .write(false, None)
        .expect("write");

    // text escapes `<`, `>` and `&` only, as xmlbuilder2 does
    let body = fs::read_to_string(dir.path().join(TV_SHOW_FILE)).expect("nfo");
    assert!(
        body.contains("<title>A &lt;b&gt; &amp; 'c' \"d\"</title>"),
        "{body}"
    );
    let events = testing::events(body.as_bytes());
    assert!(
        events.contains(&"text \"A <b> & 'c' \\\"d\\\"\"".to_string()),
        "{events:?}"
    );
}

#[test]
fn tv_show_writes_poster_from_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut nfo = TvShowNfo::new(show(vec![id("1", "anidb", true)]), dir.path()).expect("dir");
    nfo.write(false, Some(b"\xff\xd8JPEG")).expect("write");

    let poster = dir.path().join(POSTER_FILE);
    assert_eq!(fs::read(&poster).expect("poster"), b"\xff\xd8JPEG");
    assert_eq!(mode_of(&poster), 0o644);
    assert_eq!(mode_of(&dir.path().join(TV_SHOW_FILE)), 0o644);
}

#[test]
fn tv_show_keeps_existing_poster_unless_overwrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let poster = dir.path().join(POSTER_FILE);
    let mut nfo = TvShowNfo::new(show(vec![id("1", "anidb", true)]), dir.path()).expect("dir");
    assert!(nfo.needs_poster(false), "no poster yet, so fetch one");

    fs::write(&poster, "OLD").expect("seed");
    assert!(!nfo.needs_poster(false), "present, so skip the download");
    assert!(nfo.needs_poster(true), "overwrite replaces it");

    nfo.write(false, Some(b"NEW")).expect("write");
    assert_eq!(fs::read(&poster).expect("poster"), b"OLD");

    nfo.write(true, Some(b"NEW")).expect("overwrite");
    assert_eq!(fs::read(&poster).expect("poster"), b"NEW");
    assert_eq!(mode_of(&poster), 0o644);
}

#[test]
fn tv_show_without_poster_bytes_writes_no_poster() {
    let dir = tempfile::tempdir().expect("tempdir");
    TvShowNfo::new(show(vec![id("1", "anidb", true)]), dir.path())
        .expect("dir")
        .write(false, None)
        .expect("write");
    assert!(dir.path().join(TV_SHOW_FILE).exists());
    assert!(!dir.path().join(POSTER_FILE).exists());
}

#[test]
fn overwrite_without_poster_bytes_keeps_an_existing_poster() {
    // poster fetching can be disabled; --force must then leave poster.jpg alone
    let dir = tempfile::tempdir().expect("tempdir");
    let poster = dir.path().join(POSTER_FILE);
    fs::write(&poster, "KEEP").expect("seed");
    TvShowNfo::new(show(vec![id("1", "anidb", true)]), dir.path())
        .expect("dir")
        .write(true, None)
        .expect("write");
    assert_eq!(fs::read(&poster).expect("poster"), b"KEEP");
}

#[test]
fn tv_show_poster_write_failure_is_an_io_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    // a directory where the poster should go makes the rename fail
    fs::create_dir(dir.path().join(POSTER_FILE)).expect("blocker");
    let mut nfo = TvShowNfo::new(show(vec![id("1", "anidb", true)]), dir.path()).expect("dir");

    let err = nfo
        .write(true, Some(b"JPEG"))
        .expect_err("poster cannot be written");
    assert!(
        matches!(&err, NfoError::Io { path, .. } if path.ends_with(POSTER_FILE)),
        "{err:?}"
    );
    // the NFO was written first, and no temporary file is left behind
    assert!(dir.path().join(TV_SHOW_FILE).exists());
    let mut names: Vec<_> = fs::read_dir(dir.path())
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, [POSTER_FILE, TV_SHOW_FILE]);
}

#[test]
fn tv_show_is_utf8_and_keeps_cjk_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    let title = "進撃の巨人";
    let original = "ゆるキャン△カタカナ";
    let plot = "漢字・ひらがな・カタカナ & 記号「引用」";
    let mut tv = show(vec![id("1", "anidb", true)]);
    tv.title = title.to_string();
    tv.original_title = Some(original.to_string());
    tv.plot = Some(plot.to_string());
    TvShowNfo::new(tv, dir.path())
        .expect("dir")
        .write(false, None)
        .expect("write");

    let bytes = fs::read(dir.path().join(TV_SHOW_FILE)).expect("nfo");
    let text = String::from_utf8(bytes.clone()).expect("valid UTF-8");
    assert!(
        text.starts_with(r#"<?xml version="1.0" encoding="utf-8"?>"#),
        "{text}"
    );
    // the raw bytes are the UTF-8 encoding of each string
    for expected in [
        title,
        original,
        "漢字・ひらがな・カタカナ &amp; 記号「引用」",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }

    // and a parser reads back exactly the same text
    let events = testing::events(&bytes);
    for expected in [title, original, plot] {
        let wanted = format!("text {expected:?}");
        assert!(events.contains(&wanted), "{wanted} not in {events:?}");
    }
}

#[test]
fn episode_validates_path_and_data() {
    let dir = tempfile::tempdir().expect("tempdir");
    for bad in ["/nonexistent/x.mkv", dir.path().to_str().expect("utf-8")] {
        let err = EpisodeNfo::new(vec![episode(vec![id("1", "anidb", true)])], bad)
            .expect_err("not a file");
        assert!(matches!(err, NfoError::NotAFile(_)));
        assert!(err.to_string().contains("is not a file"));
    }

    let file = episode_file(dir.path());
    let valid = |episodes: Vec<Episode>| EpisodeNfo::new(episodes, &file).expect("file").is_valid();
    assert!(!valid(vec![]));
    assert!(valid(vec![episode(vec![id("1", "anidb", true)])]));
    let mut bad_date = episode(vec![id("1", "anidb", true)]);
    bad_date.premiered = Some("nope".to_string());
    assert!(!valid(vec![bad_date]));
    assert!(!valid(vec![episode(vec![
        id("1", "anidb", true),
        id("2", "anilist", true),
    ])]));
}

#[test]
fn episode_write_writes_several_episodedetails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = episode_file(dir.path());
    let nfo_path = dir.path().join("Show - 01 - First.nfo");

    let mut invalid = EpisodeNfo::new(vec![episode(vec![])], &file).expect("file");
    assert!(matches!(
        invalid.write(false).expect_err("invalid"),
        NfoError::Invalid(_)
    ));

    let mut first = episode(vec![id("1", "anidb", true)]);
    first.original_title = Some("オリジナル".to_string());
    first.season = Some(1);
    first.episode = Some(1);
    let mut second = episode(vec![id("2", "anidb", true)]);
    second.title = "Second".to_string();
    second.episode = Some(2);
    EpisodeNfo::new(vec![first, second], &file)
        .expect("file")
        .write(false)
        .expect("write");

    let text = fs::read_to_string(&nfo_path).expect("nfo");
    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n"));
    assert_eq!(text.matches("<episodedetails>").count(), 2);
    assert!(text.contains("<title>Second</title>"));
    assert!(text.contains("<originaltitle>オリジナル</originaltitle>"));
    assert!(text.contains("<season>1</season>"));
    assert!(text.contains(r#"type="AniDB" default="true""#));

    // an existing NFO is kept unless overwrite is set
    fs::write(&nfo_path, "old").expect("seed");
    EpisodeNfo::new(vec![episode(vec![id("1", "anidb", true)])], &file)
        .expect("file")
        .write(false)
        .expect("write");
    assert_eq!(fs::read_to_string(&nfo_path).expect("read"), "old");
    EpisodeNfo::new(vec![episode(vec![id("1", "anidb", true)])], &file)
        .expect("file")
        .write(true)
        .expect("overwrite");
    assert!(fs::read_to_string(&nfo_path)
        .expect("read")
        .contains("<episodedetails>"));
}

#[test]
fn jellyfin_id_types() {
    assert_eq!(jellyfin_id_type("anidb"), "AniDB");
    assert_eq!(jellyfin_id_type("anilist"), "AniList");
    assert_eq!(jellyfin_id_type("tmdb"), "tmdb");
}

#[test]
fn iso_dates() {
    for ok in ["2020-01-02", "1999-12-31", "2020-02-30"] {
        assert!(is_iso_date(ok), "{ok}");
    }
    for bad in [
        "2020-01",
        "2020-13-01",
        "2020-00-10",
        "2020-01-32",
        "x2020-01-02",
        "2020/01/02",
        "",
    ] {
        assert!(!is_iso_date(bad), "{bad}");
    }
}
