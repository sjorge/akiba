//! The `rename` command driven in this process. `AniDB` is never contacted: the
//! known files are served from the file cache (seeded with a record keyed by
//! their ed2k hash), so no UDP traffic is needed. The unknown-hash case needs a
//! fake UDP server and is covered by the renamer tests in `akiba-core`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use akiba_core::config::Config;
use akiba_test_support::{Env, Outcome};

use super::*;

fn rename(env: &Env, args: &[&str]) -> Outcome {
    env.run::<Args>(args, run)
}

/// `rename` with the path as its last argument.
fn rename_path(env: &Env, flags: &[&str], path: &Path) -> Outcome {
    let path = path.display().to_string();
    let mut args = flags.to_vec();
    args.push(&path);
    rename(env, &args)
}

/// Writes the config the rename command needs, with a plain target format.
fn configure(target: &Path) {
    let mut config = Config::load().expect("config");
    config.anidb.http_client.name = Some("http".to_owned());
    config.anidb.http_client.version = Some(1);
    config.anidb.udp_client.name = Some("client".to_owned());
    config.anidb.udp_client.version = Some(1);
    config.anidb.udp_client.username = Some("u".to_owned());
    config.anidb.udp_client.password = Some("p".to_owned());
    config.renamer.format = "{anime_name_romaji}.{filetype}".to_owned();
    config.renamer.target_path = Some(target.to_path_buf());
    config.save().expect("save config");
}

/// The ed2k hash of `file`, the one `--print-ed2klinks` reports in its link.
fn ed2k(file: &Path) -> String {
    ed2k::hash_file(file, ed2k::Select::default())
        .expect("hash")
        .ed2k
}

fn seed(env: &Env, hash: &str, name: &str) {
    let dir = env.cache_dir().join("anidb");
    fs::create_dir_all(&dir).expect("cache dir");
    fs::write(
        dir.join(format!("{hash}.fid.toml")),
        format!("ed2k = \"{hash}\"\nanime_name_romaji = \"{name}\"\nfiletype = \"mkv\"\n"),
    )
    .expect("seed");
}

#[test]
fn wait_hint_is_only_added_when_the_message_lacks_it() {
    let banned = AnidbError::from_reply(ResponseCode::Banned, "BANNED");
    assert_eq!(udp_error_text(&banned), "banned: BANNED");
    let outage = AnidbError::from_reply(ResponseCode::AnidbOutOfService, "OUT");
    assert_eq!(udp_error_text(&outage), "server error: OUT");
    let busy = AnidbError::from_reply(ResponseCode::ServerBusy, "BUSY");
    assert_eq!(
        udp_error_text(&busy),
        "server error: BUSY, try again in 30 minutes."
    );
    let timeout = AnidbError::Timeout {
        message: "no reply".to_owned(),
    };
    assert_eq!(
        udp_error_text(&timeout),
        "timed out: no reply, try again in 30 minutes."
    );
    let missing = AnidbError::from_reply(ResponseCode::NoSuchFile, "NO SUCH FILE");
    assert_eq!(
        udp_error_text(&missing),
        "NO SUCH FILE, try again in 30 minutes."
    );
}

#[test]
fn rename_dry_run_reports_the_destination_and_moves_nothing() {
    let env = Env::new();
    let target = env.dir("target");
    let show = env.dir("show");
    configure(&target);
    let file = show.join("b.mkv");
    fs::write(&file, "known").expect("file");
    seed(&env, &ed2k(&file), "Test Anime");

    let out = rename_path(&env, &["--dry-run"], &show);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains(&format!(
            "b.mkv: Moved to {}",
            target.join("Test Anime.mkv").display()
        )),
        "{}",
        out.stdout
    );
    assert!(file.exists());
    assert!(!target.join("Test Anime.mkv").exists());
}

#[test]
fn rename_moves_copies_and_keeps_existing_destinations() {
    let env = Env::new();
    let target = env.dir("target");
    let show = env.dir("show");
    configure(&target);
    let file = show.join("b.mkv");
    fs::write(&file, "known").expect("file");
    seed(&env, &ed2k(&file), "Test Anime");
    let destination = target.join("Test Anime.mkv");

    assert_eq!(rename_path(&env, &["--copy"], &file).code, 0);
    assert!(file.exists() && destination.exists());

    // the destination exists: the file stays without --force
    fs::write(&destination, "other").expect("destination");
    assert_eq!(rename_path(&env, &[], &file).code, 0);
    assert!(file.exists());
    assert_eq!(fs::read_to_string(&destination).expect("read"), "other");

    assert_eq!(rename_path(&env, &["--force"], &file).code, 0);
    assert!(!file.exists());
    assert_eq!(fs::read_to_string(&destination).expect("read"), "known");
}

#[test]
fn rename_ignores_other_files_and_reports_an_empty_directory() {
    let env = Env::new();
    let target = env.dir("target");
    let show = env.dir("show");
    configure(&target);
    fs::write(show.join("poster.jpg"), "x").expect("file");
    fs::write(show.join("tvshow.nfo"), "x").expect("file");
    fs::write(show.join("notes.txt"), "x").expect("file");

    let out = rename_path(&env, &[], &show);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("Nothing to rename ..."));
}

#[test]
fn rename_fails_for_a_missing_path_and_without_configuration() {
    let env = Env::new();
    let out = rename(&env, &["/nope/nothing"]);
    assert_eq!(out.code, 1);
    assert!(
        out.stdout.contains("AniDB UDP Client not configured!"),
        "{}",
        out.stdout
    );

    let target = env.dir("target");
    configure(&target);
    let out = rename(&env, &["/nope/nothing"]);
    assert_eq!(out.code, 1);
    assert!(
        out.stdout
            .contains("Directory or file \"/nope/nothing\" does not exist!"),
        "{}",
        out.stdout
    );
}

#[test]
fn rename_rejects_conflicting_and_invalid_options() {
    let env = Env::new();
    let dir = env.dir("show");
    let path = dir.display().to_string();
    for flags in [
        vec!["--symlink", "--copy"],
        vec!["--print-ed2klinks", "--force"],
        vec!["--mylist-state", "nowhere"],
        vec!["--format", "{nope}"],
        vec!["--target-path", "/does/not/exist"],
    ] {
        let mut args = flags.clone();
        args.push(&path);
        assert_eq!(rename(&env, &args).code, 1, "{flags:?}");
    }
}

/// Writes `count` episode files with distinct contents (so their hashes differ).
fn episodes(show: &Path, count: usize) -> Vec<PathBuf> {
    (1..=count)
        .map(|number| {
            let file = show.join(format!("ep{number:02}.mkv"));
            fs::write(&file, format!("episode {number}")).expect("file");
            file
        })
        .collect()
}

#[test]
fn rename_continues_after_a_file_fails_to_rename() {
    let env = Env::new();
    let target = env.dir("target");
    let show = env.dir("show");
    configure(&target);
    let files = episodes(&show, 5);
    for (index, file) in files.iter().enumerate() {
        seed(&env, &ed2k(file), &format!("Show {}", index + 1));
    }
    // a regular file where the destination directory of episode 3 belongs, so
    // only that file cannot be renamed
    fs::write(target.join("Show 3"), "in the way").expect("blocker");

    let out = rename_path(
        &env,
        &[
            "--format",
            "{anime_name_romaji}/{anime_name_romaji}.{filetype}",
        ],
        &show,
    );
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(!out.stdout.contains("Stopped"), "{}", out.stdout);
    assert!(
        out.stdout.contains("ep03.mkv: Renaming ..."),
        "{}",
        out.stdout
    );

    for (index, file) in files.iter().enumerate() {
        let number = index + 1;
        if number == 3 {
            assert!(file.exists(), "episode 3 must stay in place");
        } else {
            assert!(!file.exists(), "episode {number} was not renamed");
            let destination = target
                .join(format!("Show {number}"))
                .join(format!("Show {number}.mkv"));
            assert!(destination.exists(), "{}", destination.display());
        }
    }
}

#[test]
fn rename_continues_after_a_file_cannot_be_read() {
    let env = Env::new();
    let target = env.dir("target");
    let show = env.dir("show");
    configure(&target);
    let files = episodes(&show, 5);
    for (index, file) in files.iter().enumerate() {
        if index != 2 {
            seed(&env, &ed2k(file), &format!("Show {}", index + 1));
        }
    }
    let unreadable = &files[2];
    fs::set_permissions(unreadable, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::File::open(unreadable).is_ok() {
        // running as root: permissions do not apply, so nothing can fail here
        eprintln!("skipped: the file stays readable (root?)");
        return;
    }

    let out = rename_path(&env, &[], &show);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(!out.stdout.contains("Stopped"), "{}", out.stdout);

    assert!(unreadable.exists(), "the unreadable file stays in place");
    for (index, file) in files.iter().enumerate() {
        if index != 2 {
            let number = index + 1;
            assert!(!file.exists(), "episode {number} was not renamed");
            assert!(target.join(format!("Show {number}.mkv")).exists());
        }
    }
}

#[test]
fn rename_moves_the_sidecar_nfo_with_the_episode() {
    let env = Env::new();
    let target = env.dir("target");
    let show = env.dir("show");
    configure(&target);
    let file = show.join("b.mkv");
    fs::write(&file, "known").expect("file");
    let nfo = show.join("b.nfo");
    fs::write(&nfo, "<episodedetails/>").expect("nfo");
    fs::set_permissions(&nfo, fs::Permissions::from_mode(0o400)).expect("chmod");
    seed(&env, &ed2k(&file), "Test Anime");

    let out = rename_path(&env, &[], &show);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let stdout = out.stdout;
    let video = target.join("Test Anime.mkv");
    let moved_nfo = target.join("Test Anime.nfo");
    assert!(video.exists() && moved_nfo.exists());
    assert!(!file.exists() && !nfo.exists());
    assert_eq!(
        fs::read_to_string(&moved_nfo).expect("read"),
        "<episodedetails/>"
    );
    let video_line = format!("b.mkv: Moved to {}!", video.display());
    let nfo_line = format!("b.nfo: Moved to {}!", moved_nfo.display());
    let video_at = stdout.find(&video_line).expect(&stdout);
    let nfo_at = stdout.find(&nfo_line).expect(&stdout);
    assert!(video_at < nfo_at, "{stdout}");
}

#[test]
fn rename_moves_the_subtitles_with_the_episode() {
    let env = Env::new();
    let target = env.dir("target");
    let show = env.dir("show");
    configure(&target);
    let file = show.join("b.mkv");
    fs::write(&file, "known").expect("file");
    fs::write(show.join("b.srt"), "plain").expect("srt");
    fs::write(show.join("b.en.srt"), "english").expect("srt");
    fs::write(show.join("b.nfo"), "<episodedetails/>").expect("nfo");
    // not a sidecar of b.mkv: another episode number
    fs::write(show.join("b 10.srt"), "other").expect("srt");
    seed(&env, &ed2k(&file), "Test Anime");

    let out = rename_path(&env, &[], &show);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let stdout = out.stdout;
    let video = target.join("Test Anime.mkv");
    assert!(video.exists());
    assert_eq!(
        fs::read_to_string(target.join("Test Anime.srt")).expect("read"),
        "plain"
    );
    assert_eq!(
        fs::read_to_string(target.join("Test Anime.en.srt")).expect("read"),
        "english"
    );
    assert!(target.join("Test Anime.nfo").exists());
    assert!(!show.join("b.srt").exists() && !show.join("b.en.srt").exists());
    assert!(show.join("b 10.srt").exists());

    let video_at = stdout
        .find(&format!("b.mkv: Moved to {}!", video.display()))
        .expect(&stdout);
    for line in [
        format!(
            "b.srt: Moved to {}!",
            target.join("Test Anime.srt").display()
        ),
        format!(
            "b.en.srt: Moved to {}!",
            target.join("Test Anime.en.srt").display()
        ),
    ] {
        let at = stdout.find(&line).expect(&stdout);
        assert!(video_at < at, "{stdout}");
    }
}
