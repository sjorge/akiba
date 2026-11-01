//! The flags of `configure`, driven in this process. The dump goes straight to
//! stdout, past the logger, so these tests leave `--dump` out and read the saved
//! configuration back instead. The end-to-end test in `akiba` checks the dump.

use std::fs;

use akiba_core::config::Config;
use akiba_test_support::{Env, Outcome};

use super::*;

const CA_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../anidb-http/tests/fixtures/test-ca.pem"
);

fn configure(env: &Env, args: &[&str]) -> Outcome {
    env.run::<Args>(args, run)
}

/// The saved configuration as JSON, like `--dump` prints it.
fn saved() -> serde_json::Value {
    serde_json::to_value(Config::load().expect("config")).expect("json")
}

#[test]
fn client_version_must_be_a_whole_number() {
    assert_eq!(split_client("akiba/2"), Some(("akiba".to_owned(), 2)));
    assert_eq!(split_client("akiba"), Some(("akiba".to_owned(), 1)));
    assert_eq!(split_client("akiba/ 3 "), Some(("akiba".to_owned(), 3)));
    for bad in ["akiba/1x", "akiba/x", "akiba/", "akiba/-1", "akiba/1.5"] {
        assert_eq!(split_client(bad), None, "{bad}");
    }
    assert!(client_arg("akiba/1x").is_err());
}

#[test]
fn invalid_rename_format_is_rejected() {
    let env = Env::new();
    let out = configure(&env, &["--rename-format", "{nope}"]);
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("The tag {nope} is not known"),
        "{}",
        out.stderr
    );
    assert!(!env.config_file().exists());
}

#[test]
fn configure_accepts_every_flag() {
    let env = Env::new();
    let target = env.dir("target");
    let out = configure(
        &env,
        &[
            "--anidb-http-client",
            "web/3",
            "--anidb-udp-client",
            "client",
            "--anidb-auth-username",
            "user",
            "--anidb-auth-password",
            "secret",
            "--anidb-auth-api-key",
            "key",
            "--anidb-poster",
            "--anilist-token",
            "token",
            "--tmdb-api-key",
            "tmdbkey",
            "--overwrite-nfo",
            "--rename-format",
            "{anime_name_romaji}.{filetype}",
            "--rename-target-path",
            &format!("{}/", target.display()),
        ],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let dump = saved();
    assert_eq!(dump["anidb"]["http_client"]["name"], "web");
    assert_eq!(dump["anidb"]["http_client"]["version"], 3);
    // a missing version is 1
    assert_eq!(dump["anidb"]["udp_client"]["version"], 1);
    assert_eq!(dump["anidb"]["udp_client"]["username"], "user");
    assert_eq!(dump["anidb"]["udp_client"]["password"], "secret");
    assert_eq!(dump["anidb"]["udp_client"]["api_key"], "key");
    assert_eq!(dump["anidb"]["poster"], true);
    assert_eq!(dump["anilist"]["token"], "token");
    assert_eq!(dump["tmdb"]["api_key"], "tmdbkey");
    assert_eq!(dump["overwrite_nfo"], true);
    assert_eq!(dump["renamer"]["format"], "{anime_name_romaji}.{filetype}");
    // normalised without the trailing slash
    assert_eq!(dump["renamer"]["target_path"], target.display().to_string());

    // the negations undo them, and the last one given wins
    let out = configure(
        &env,
        &[
            "--no-anidb-auth-api-key",
            "--no-anidb-poster",
            "--no-overwrite-nfo",
            "--no-rename-target-path",
            "--anidb-poster",
            "--no-anidb-poster",
        ],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let dump = saved();
    assert!(dump["anidb"]["udp_client"].get("api_key").is_none());
    assert_eq!(dump["anidb"]["poster"], false);
    assert_eq!(dump["overwrite_nfo"], false);
    assert!(dump["renamer"].get("target_path").is_none());
}

#[test]
fn configure_rejects_bad_values() {
    let env = Env::new();
    assert_eq!(configure(&env, &["--anidb-udp-client", "test/x"]).code, 1);
    assert_eq!(
        configure(&env, &["--rename-target-path", "/does/not/exist"]).code,
        1
    );
}

#[test]
fn configure_rejects_udp_client_names_outside_the_rule() {
    let env = Env::new();
    // 3 and 17 letters, upper case, a digit, a hyphen, empty, and no name
    for value in [
        "abc/1",
        "abcdefghijklmnopq/1",
        "Client/1",
        "client2/1",
        "akiba-test/1",
        "",
        "/1",
    ] {
        let out = configure(&env, &["--anidb-udp-client", value]);
        assert_eq!(out.code, 1, "{value}");
        assert!(
            out.stderr
                .contains("Expecting 4 to 16 lower case letters (a-z) for the client name."),
            "{value}: {}",
            out.stderr
        );
        assert!(!env.config_file().exists(), "{value}");
    }
}

#[test]
fn configure_accepts_udp_client_names_inside_the_rule() {
    let env = Env::new();
    // the exit code stays 1 (the rest of the config is missing), so only the
    // stored names are checked
    for (value, name, version) in [
        ("client/1", "client", 1),
        ("akiba/3", "akiba", 3),
        ("abcdefghijklmnop/2", "abcdefghijklmnop", 2),
        ("akiba", "akiba", 1),
    ] {
        let out = configure(&env, &["--anidb-udp-client", value]);
        assert!(
            !out.stderr.contains("lower case letters"),
            "{value}: {}",
            out.stderr
        );
        let dump = saved();
        assert_eq!(dump["anidb"]["udp_client"]["name"], name, "{value}");
        assert_eq!(dump["anidb"]["udp_client"]["version"], version, "{value}");
    }

    // the HTTP client name is not checked against the UDP rule
    let out = configure(&env, &["--anidb-http-client", "akiba-test/1"]);
    assert!(!out.stderr.contains("lower case letters"), "{}", out.stderr);
    assert_eq!(saved()["anidb"]["http_client"]["name"], "akiba-test");
}

#[test]
fn configure_sets_dumps_and_clears_the_tls_ca_file() {
    let env = Env::new();
    // the rest of the configuration is still missing
    let out = configure(&env, &["--tls-ca-file", CA_FIXTURE]);
    assert_eq!(out.code, 1);
    let file = fs::canonicalize(CA_FIXTURE).expect("fixture");
    let shown = saved()["tls"]["ca_file"]
        .as_str()
        .expect("ca_file")
        .to_owned();
    assert_eq!(fs::canonicalize(&shown).expect("path"), file);
    let on_disk = fs::read_to_string(env.config_file()).expect("config");
    assert!(
        on_disk.contains("[tls]") && on_disk.contains("ca_file"),
        "{on_disk}"
    );

    // an empty value changes nothing
    let out = configure(&env, &["--tls-ca-file", ""]);
    assert_eq!(out.code, 1);
    assert_eq!(saved()["tls"]["ca_file"], shown.as_str());

    // the negation switches back to automatic, and the last one given wins
    let out = configure(&env, &["--no-tls-ca-file"]);
    assert_eq!(out.code, 1);
    assert!(saved()["tls"].get("ca_file").is_none());
    let out = configure(&env, &["--tls-ca-file", CA_FIXTURE, "--no-tls-ca-file"]);
    assert_eq!(out.code, 1);
    assert!(saved()["tls"].get("ca_file").is_none());
}

#[test]
fn configure_rejects_a_tls_ca_file_that_is_missing_or_not_pem() {
    let env = Env::new();
    let text_file = env.root().join("not-pem.txt");
    fs::write(&text_file, "hello\n").expect("write");
    let empty = env.root().join("empty.pem");
    fs::write(&empty, "").expect("write");
    for value in [
        "/does/not/exist.pem".to_owned(),
        text_file.display().to_string(),
        empty.display().to_string(),
        env.root().display().to_string(),
    ] {
        let out = configure(&env, &["--tls-ca-file", &value]);
        assert_eq!(out.code, 1, "{value}");
        assert!(out.stderr.contains("CA file"), "{value}: {}", out.stderr);
    }
    assert!(!env.config_file().exists());
}

#[test]
fn a_config_without_the_tls_table_still_works_and_keeps_unknown_tls_keys() {
    let env = Env::new();
    fs::create_dir_all(env.config_file().parent().expect("dir")).expect("dir");
    fs::write(
        env.config_file(),
        "[anidb.http_client]\nname = \"web\"\nversion = 3\n",
    )
    .expect("write");
    // the UDP client is still missing
    let out = configure(&env, &[]);
    assert_eq!(out.code, 1);
    assert!(saved()["tls"].get("ca_file").is_none());

    fs::write(env.config_file(), "[tls]\nfuture = 1\n").expect("write");
    let out = configure(&env, &["--tls-ca-file", CA_FIXTURE]);
    assert_eq!(out.code, 1);
    let on_disk = fs::read_to_string(env.config_file()).expect("config");
    assert!(on_disk.contains("future = 1"), "{on_disk}");
}
