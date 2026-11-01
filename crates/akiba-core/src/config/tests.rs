use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serial_test::serial;

use super::*;
use crate::testutil::{EnvGuard, TempEnv};

fn mode(path: &Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

fn with_http(name: &str, version: u32) -> Config {
    let mut config = Config::default();
    config.anidb.http_client.name = Some(name.to_owned());
    config.anidb.http_client.version = Some(version);
    config
}

fn with_udp(name: Option<&str>, version: Option<u32>) -> Config {
    let mut config = Config::default();
    config.anidb.udp_client.name = name.map(str::to_owned);
    config.anidb.udp_client.version = version;
    config
}

fn messages(config: &Config, command: Command) -> Vec<&'static str> {
    match config.validate(command) {
        Ok(()) => Vec::new(),
        Err(problems) => problems.iter().map(|p| p.message).collect(),
    }
}

#[test]
#[serial]
fn load_returns_defaults_and_merges_a_file() {
    let env = TempEnv::new();
    let defaults = Config::load().expect("defaults");
    assert_eq!(
        defaults.anidb.http_client.url,
        "http://api.anidb.net:9001/httpapi"
    );
    assert!(!defaults.anidb.poster);
    assert_eq!(defaults.cache.path, env.root().join("cache").join("akiba"));
    assert_eq!(defaults.cache.metadata_age, 90);
    assert_eq!(defaults.cache.mapping_age, 7);
    assert_eq!(defaults.cache.title_age, 3);
    assert_eq!(defaults.cache.hash_age, 30);
    assert!(defaults.renamer.format.contains("{anime_name_romaji}"));
    assert_eq!(defaults.renamer.format, DEFAULT_FORMAT);
    assert!(!defaults.overwrite_nfo);

    let file = paths::config_file().expect("config file");
    fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
    fs::write(
        &file,
        "[anidb.udp_client]\nname = \"client\"\nversion = 2\n\n[cache]\nmapping_age = 1\n",
    )
    .expect("write");

    let merged = Config::load().expect("merged");
    assert_eq!(
        merged.anidb.udp_client,
        UdpClientConfig {
            name: Some("client".to_owned()),
            version: Some(2),
            ..UdpClientConfig::default()
        }
    );
    assert_eq!(merged.cache.mapping_age, 1);
    // untouched defaults survive the merge
    assert_eq!(merged.cache.hash_age, 30);
    assert_eq!(
        merged.anidb.http_client.url,
        "http://api.anidb.net:9001/httpapi"
    );
}

#[test]
#[serial]
fn explicit_cache_path_is_kept() {
    let env = TempEnv::new();
    let file = env.root().join("custom.toml");
    fs::write(&file, "[cache]\npath = \"/srv/cache\"\n").expect("write");

    let config = Config::load_from(&file).expect("load");
    assert_eq!(config.cache.path, PathBuf::from("/srv/cache"));
}

#[test]
#[serial]
fn save_creates_a_private_file() {
    let _env = TempEnv::new();
    let file = paths::config_file().expect("config file");
    assert!(!file.parent().expect("parent").exists());

    Config::load().expect("load").save().expect("save");
    // NOTE: regression for creating the config file with mode 0600
    assert_eq!(mode(&file), 0o600);
    let dir_mode = mode(file.parent().expect("parent"));
    assert_eq!(
        dir_mode & !0o750,
        0,
        "dir mode {dir_mode:o} is wider than 0750"
    );

    // an existing file is tightened
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).expect("chmod");
    let mut config = Config::load().expect("load");
    config.anilist.token = Some("secret".to_owned());
    config.renamer.target_path = Some(PathBuf::from("/media/anime"));
    config.save().expect("save");
    assert_eq!(mode(&file), 0o600);

    // and it round trips
    let read = Config::load().expect("reload");
    assert_eq!(read.anilist.token.as_deref(), Some("secret"));
    assert_eq!(
        read.renamer.target_path,
        Some(PathBuf::from("/media/anime"))
    );
}

#[test]
#[serial]
fn unknown_keys_survive_a_save() {
    let env = TempEnv::new();
    let file = env.root().join("akiba.toml");
    fs::write(
            &file,
            "future_flag = true\n\n[anidb]\nmystery = \"x\"\n\n[anidb.udp_client]\nextra_count = 3\n\n[cache]\nnew_age = 5\n",
        )
        .expect("write");

    let config = Config::load_from(&file).expect("load");
    config.save_to(&file).expect("save");

    let text = fs::read_to_string(&file).expect("read");
    let saved: toml::Table = toml::from_str(&text).expect("parse");
    assert_eq!(saved["future_flag"].as_bool(), Some(true));
    assert_eq!(saved["anidb"]["mystery"].as_str(), Some("x"));
    assert_eq!(
        saved["anidb"]["udp_client"]["extra_count"].as_integer(),
        Some(3)
    );
    assert_eq!(saved["cache"]["new_age"].as_integer(), Some(5));
}

#[test]
fn the_tls_key_is_optional_and_empty_means_automatic() {
    let config: Config = toml::from_str("[anidb]\nposter = true\n").expect("parse");
    assert_eq!(config.tls.ca_file, None);
    assert_eq!(config.tls_options(), TlsOptions::automatic());

    let config: Config = toml::from_str("[tls]\nca_file = \"\"\n").expect("parse");
    assert_eq!(config.tls_options(), TlsOptions::automatic());

    let config: Config = toml::from_str("[tls]\nca_file = \"/etc/ca.pem\"\n").expect("parse");
    assert_eq!(
        config.tls_options().ca_file(),
        Some(Path::new("/etc/ca.pem"))
    );
    // an unset key is not written back
    let text = toml::to_string(&Config::default()).expect("serialise");
    assert!(!text.contains("ca_file"), "{text}");
}

#[test]
fn validate_write_nfo_checks_the_http_client() {
    assert_eq!(
        messages(&Config::default(), Command::WriteNfo),
        [
            "AniDB HTTP Client not configured! Please set with --anidb-http-client",
            "Please register a project on https://anidb.net/software/add, and add a HTTP client",
        ]
    );
    assert!(with_http("c", 1).validate(Command::WriteNfo).is_ok());

    let mut name_only = Config::default();
    name_only.anidb.http_client.name = Some("c".to_owned());
    assert_eq!(messages(&name_only, Command::WriteNfo).len(), 2);

    // the commands do not depend on each other
    assert!(with_http("c", 1).validate(Command::Renamer).is_err());
}

#[test]
fn validate_write_nfo_flags_an_empty_url() {
    let mut config = with_http("c", 1);
    config.anidb.http_client.url = String::new();
    assert_eq!(
        messages(&config, Command::WriteNfo),
        ["Please configure the AniDB URL!"]
    );
}

#[test]
fn validate_renamer_checks_udp_client_and_credentials() {
    let udp_lines = [
        "AniDB UDP Client not configured! Please set with --anidb-udp-client.",
        "Please register a project on https://anidb.net/software/add, and add a UDP client",
    ];
    let creds_line = "AniDB UDP Client missing username and password configuration! Please set with --anidb-auth-username and --anidb-auth-password.";

    assert_eq!(
        messages(&Config::default(), Command::Renamer),
        [udp_lines[0], udp_lines[1], creds_line]
    );

    // a name without a version
    assert_eq!(
        messages(&with_udp(Some("c"), None), Command::Renamer),
        [udp_lines[0], udp_lines[1], creds_line]
    );

    // a client without credentials
    let mut udp_only = with_udp(Some("c"), Some(1));
    assert_eq!(messages(&udp_only, Command::Renamer), [creds_line]);

    // a complete client
    udp_only.anidb.udp_client.username = Some("u".to_owned());
    udp_only.anidb.udp_client.password = Some("p".to_owned());
    assert!(udp_only.validate(Command::Renamer).is_ok());

    // credentials without a version
    let mut no_version = with_udp(Some("c"), None);
    no_version.anidb.udp_client.username = Some("u".to_owned());
    no_version.anidb.udp_client.password = Some("p".to_owned());
    assert_eq!(messages(&no_version, Command::Renamer), udp_lines);

    // an HTTP-only config does not satisfy the renamer
    assert!(with_http("c", 1).validate(Command::Renamer).is_err());
}

#[test]
#[serial]
fn load_fails_on_a_malformed_file() {
    let env = TempEnv::new();
    let file = env.root().join("bad.toml");
    fs::write(&file, "[anidb\n").expect("write");
    let err = Config::load_from(&file).expect_err("must fail");
    assert!(matches!(err, ConfigError::Parse { .. }));
}

#[test]
#[serial]
fn load_without_any_home_fails() {
    let _guard = EnvGuard::set(&[
        ("AKIBA_CONFIG", None),
        ("XDG_CONFIG_HOME", None),
        ("HOME", None),
    ]);
    assert!(matches!(
        Config::load(),
        Err(ConfigError::Path(PathError::NoHome))
    ));
}
