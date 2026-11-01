//! The real binary: the top-level command and the exit codes, plus one end to
//! end test per command for the wiring. The flags and the behaviour of each
//! command are tested in its own `akiba-cmd-*` crate.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::{text, Env};

#[test]
fn help_lists_the_commands() {
    let env = Env::new();
    let out = env.cmd().arg("--help").assert().success();
    let stdout = text(&out.get_output().stdout);
    for command in ["configure", "write-nfo", "rename", "idmap"] {
        assert!(stdout.contains(command), "{command} missing in:\n{stdout}");
    }
}

#[test]
fn version_is_the_bare_version_number() {
    let env = Env::new();
    for flag in ["--version", "-V"] {
        let out = env.cmd().arg(flag).assert().success();
        let stdout = text(&out.get_output().stdout);
        assert_eq!(stdout.trim(), env!("CARGO_PKG_VERSION"));
    }
}

#[test]
fn no_command_and_bad_arguments_exit_with_one() {
    let env = Env::new();
    env.cmd().assert().code(1);
    env.cmd().arg("nope").assert().code(1);
    env.cmd().args(["rename"]).assert().code(1);
    env.cmd().args(["idmap", "abc"]).assert().code(1);
}

#[test]
fn configure_writes_the_config_and_dumps_it() {
    let env = Env::new();
    let out = env
        .cmd()
        .args(["configure", "--anidb-udp-client", "test/1", "--dump"])
        .assert()
        // the rest of the configuration is still missing
        .code(1);

    let file = fs::read_to_string(env.config_file()).expect("config written");
    assert!(file.contains("name = \"test\""), "{file}");
    let mode = fs::metadata(env.config_file())
        .expect("meta")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);

    // the dump is the full config, secrets included
    let stdout = text(&out.get_output().stdout);
    let dump: serde_json::Value =
        serde_json::from_str(&stdout[stdout.find('{').expect("json")..]).expect("valid json");
    assert_eq!(
        dump["anidb"]["udp_client"],
        serde_json::json!({ "name": "test", "version": 1 })
    );
}

#[test]
fn idmap_sets_shows_and_removes_mappings() {
    let env = Env::new();
    let file = env.cache_dir().join("local.map.toml");

    // the first mapping creates the cache directory and the file
    let out = env
        .cmd()
        .args(["idmap", "12", "--anilistid", "34", "--tmdbid", "56"])
        .assert()
        .success();
    let stdout = text(&out.get_output().stdout);
    assert!(
        stdout.contains("AniDB 12 => AniList 34, TheMovieDB 56:1"),
        "{stdout}"
    );
    let body = fs::read_to_string(&file).expect("mapping file");
    assert!(body.contains("anilist = 34"), "{body}");
    let mode = fs::metadata(&file).expect("meta").permissions().mode();
    assert_eq!(mode & 0o777, 0o660);

    let out = env
        .cmd()
        .args(["idmap", "12", "--no-anilistid"])
        .assert()
        .success();
    let stdout = text(&out.get_output().stdout);
    assert!(stdout.contains("Unmapping AniList ID from AniDB ID 12"));
    assert!(stdout.contains("AniList (no mapping), TheMovieDB 56:1"));

    // an entry without any mapping is dropped, --no-show stays quiet
    let out = env
        .cmd()
        .args(["idmap", "12", "--no-tmdbid", "--no-show"])
        .assert()
        .success();
    assert!(!text(&out.get_output().stdout).contains("AniDB 12 =>"));
    assert!(!fs::read_to_string(&file).expect("file").contains("12"));

    let out = env.cmd().args(["idmap", "12"]).assert().success();
    assert!(text(&out.get_output().stdout).contains("AniDB 12 has no mappings."));
}

#[test]
fn rename_and_write_nfo_list_their_flags() {
    let env = Env::new();
    let out = env.cmd().args(["rename", "--help"]).assert().success();
    let help = text(&out.get_output().stdout);
    for flag in [
        "--rehash",
        "--fresh",
        "--force",
        "--copy",
        "--symlink",
        "--mylist-state",
        "--dry-run",
        "--format",
        "--target-path",
        "--print-ed2klinks",
    ] {
        assert!(help.contains(flag), "{flag} missing in:\n{help}");
    }
    let out = env.cmd().args(["write-nfo", "--help"]).assert().success();
    let help = text(&out.get_output().stdout);
    for flag in ["--aid", "--anilistid", "--tmdbid", "--fresh", "--force"] {
        assert!(help.contains(flag), "{flag} missing in:\n{help}");
    }
    let out = env.cmd().args(["configure", "--help"]).assert().success();
    let help = text(&out.get_output().stdout);
    for flag in [
        "--anidb-http-client",
        "--anidb-udp-client",
        "--anidb-auth-username",
        "--anidb-auth-password",
        "--anidb-auth-api-key",
        "--no-anidb-auth-api-key",
        "--anidb-poster",
        "--no-anidb-poster",
        "--anilist-token",
        "--tmdb-api-key",
        "--overwrite-nfo",
        "--no-overwrite-nfo",
        "--rename-format",
        "--rename-target-path",
        "--no-rename-target-path",
        "--tls-ca-file",
        "--no-tls-ca-file",
        "--dump",
    ] {
        assert!(help.contains(flag), "{flag} missing in:\n{help}");
    }
    let out = env.cmd().args(["idmap", "--help"]).assert().success();
    let help = text(&out.get_output().stdout);
    for flag in [
        "--anilistid",
        "--no-anilistid",
        "--tmdbid",
        "--no-tmdbid",
        "--no-show",
    ] {
        assert!(help.contains(flag), "{flag} missing in:\n{help}");
    }
}

#[test]
fn a_corrupt_config_is_reported_the_same_by_every_command_that_loads_it() {
    let env = Env::new();
    fs::create_dir_all(env.config_file().parent().expect("dir")).expect("dir");
    fs::write(env.config_file(), "[[[ not toml").expect("write");
    let episodes = env.dir("episodes");
    let episodes = episodes.to_string_lossy().into_owned();
    for args in [
        vec!["configure"],
        vec!["idmap", "5"],
        vec!["rename", episodes.as_str()],
        vec!["write-nfo", episodes.as_str()],
    ] {
        let out = env.cmd().args(&args).assert().code(1);
        let stdout = text(&out.get_output().stdout);
        assert!(
            stdout.starts_with("[!!] Failed to read the configuration: cannot parse config "),
            "{args:?}: {stdout}"
        );
    }
}

#[test]
fn print_ed2klinks_only_prints_the_link() {
    let env = Env::new();
    env.cmd()
        .args([
            "configure",
            "--anidb-http-client",
            "http/1",
            "--anidb-udp-client",
            "client/1",
            "--anidb-auth-username",
            "u",
            "--anidb-auth-password",
            "p",
            "--rename-format",
            "{anime_name_romaji}.{filetype}",
            "--rename-target-path",
            &env.dir("target").display().to_string(),
        ])
        .assert()
        .success();
    let file = env.root.path().join("a.mkv");
    fs::write(&file, "known").expect("file");
    let out = env
        .cmd()
        .args(["rename", "--print-ed2klinks", &file.display().to_string()])
        .assert()
        .success();
    let stdout = text(&out.get_output().stdout);
    assert!(stdout.starts_with("ed2k://|file|a.mkv|5|"), "{stdout}");
    assert_eq!(stdout.lines().count(), 1, "no banner or log lines");
}

// `write-nfo` over the fixture cache: the titles, id list and AniDB record are
// fresh in the cache, so nothing is downloaded. The poster is switched off.
const FIXTURE_CACHE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cache");

fn seed_cache(env: &Env) {
    let cache = env.cache_dir();
    fs::create_dir_all(cache.join("anidb")).expect("cache dir");
    for file in ["titles.map.toml", "list.map.toml", "anidb/100.aid.toml"] {
        fs::copy(Path::new(FIXTURE_CACHE).join(file), cache.join(file)).expect("copy fixture");
    }
}

#[test]
fn writes_the_show_and_episode_nfo_from_the_cache() {
    let env = Env::new();
    seed_cache(&env);
    env.cmd()
        .args([
            "configure",
            "--anidb-http-client",
            "http/1",
            "--no-anidb-poster",
        ])
        .assert()
        .code(1); // the renamer part of the config is missing
    let show = env.dir("[anidb-100] Some Show");
    fs::write(show.join("Some Show - 01 - Pilot (aabbccdd).mkv"), "x").expect("episode");

    let out = env
        .cmd()
        .args(["write-nfo", &show.display().to_string()])
        .assert()
        .success();
    let stdout = text(&out.get_output().stdout);
    assert!(stdout.contains("Writing show NFO ..."), "{stdout}");

    let nfo = fs::read_to_string(show.join("tvshow.nfo")).expect("tvshow.nfo");
    assert!(nfo.contains("<tvshow>"), "{nfo}");
    assert!(nfo.contains("AniDB"), "{nfo}");
    assert!(show.join("Some Show - 01 - Pilot (aabbccdd).nfo").exists());
    assert!(!show.join("poster.jpg").exists());
}
