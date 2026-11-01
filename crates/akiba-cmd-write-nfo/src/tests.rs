use std::fs;

use akiba_core::cache::paths::list_file;
use akiba_core::cache::paths::titles_file;
use akiba_core::cache::records::{ListFile, TitlesFile};
use akiba_core::cache::KeyedMap;
use akiba_core::config::Config;
use akiba_test_support::{Env, Outcome};
use anidb_http::Title;
use clap::FromArgMatches;

use super::*;

fn write_nfo(env: &Env, args: &[&str]) -> Outcome {
    env.run::<Args>(args, run)
}

/// Writes the config the command needs: the HTTP client, no poster.
fn configure() {
    let mut config = Config::load().expect("config");
    config.anidb.http_client.name = Some("http".to_owned());
    config.anidb.http_client.version = Some(1);
    config.anidb.poster = false;
    config.save().expect("save config");
}

#[test]
fn fails_for_a_missing_directory_and_missing_configuration() {
    let env = Env::new();
    let out = write_nfo(&env, &["/nope"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("AniDB HTTP Client not configured!"));

    configure();
    let out = write_nfo(&env, &["/nope"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("Directory \"/nope\" does not exist!"));
}

#[test]
fn rejects_bad_ids() {
    let env = Env::new();
    assert_eq!(write_nfo(&env, &["/tmp", "--aid", "abc"]).code, 1);
}

/// Parses `args` the way the binary does, without running the command.
fn parse(args: &[&str]) -> Result<Args, clap::Error> {
    let command = Args::augment_args(clap::Command::new("akiba-test"));
    let matches =
        command.try_get_matches_from(std::iter::once("akiba-test").chain(args.iter().copied()))?;
    Args::from_arg_matches(&matches)
}

#[test]
fn id_flags_take_negative_numbers_and_the_marker() {
    assert_eq!(parse(&["show", "--tmdbid", "-1"]).unwrap().tmdbid, Some(-1));
    assert_eq!(
        parse(&["show", "--tmdbid", "blacklist"]).unwrap().tmdbid,
        Some(-1)
    );
    assert_eq!(
        parse(&["show", "--tmdbid", "259140"]).unwrap().tmdbid,
        Some(259_140)
    );
    assert_eq!(
        parse(&["show", "--anilistid", "-1"]).unwrap().anilistid,
        Some(-1)
    );
    assert_eq!(
        parse(&["show", "--anilistid", "blacklist"])
            .unwrap()
            .anilistid,
        Some(-1)
    );
    assert_eq!(parse(&["show", "--aid", "5"]).unwrap().aid, Some(5));
}

#[test]
fn id_flags_reject_other_spellings_of_the_marker() {
    for option in ["--tmdbid", "--anilistid"] {
        assert!(parse(&["show", option, "Blacklist"]).is_err(), "{option}");
    }
}

/// Seeds the caches a run reads, so it needs neither the network nor a title
/// lookup: empty title and list maps and the `AniDB` show for `aid`.
fn seed_caches(env: &Env, aid: i64) {
    let cache = env.cache_dir();
    CacheFile::<TitlesFile>::new(titles_file(&cache), None)
        .store(&KeyedMap::default())
        .expect("titles cache");
    CacheFile::<ListFile>::new(list_file(&cache), None)
        .store(&KeyedMap::default())
        .expect("list cache");
    let show = AniDbShow {
        id: aid,
        titles: vec![Title {
            title: "Test Show".to_owned(),
            kind: Some("main".to_owned()),
            language: "x-jat".to_owned(),
        }],
        ..AniDbShow::default()
    };
    CacheFile::<AniDbShow>::new(aid_file(&cache, aid), None)
        .store(&show)
        .expect("show cache");
}

#[test]
fn tmdb_blacklist_writes_no_tmdb_uniqueid() {
    let env = Env::new();
    configure();
    seed_caches(&env, 5);
    let blacklisted = env.dir("blacklisted");
    let out = write_nfo(
        &env,
        &[
            blacklisted.to_str().expect("utf-8"),
            "--aid",
            "5",
            "--tmdbid",
            "blacklist",
        ],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let nfo = fs::read_to_string(blacklisted.join("tvshow.nfo")).expect("tvshow.nfo");
    assert!(!nfo.contains("type=\"tmdb\""), "{nfo}");

    // the same run with a real id does write one, so the check above means something
    let mapped = env.dir("mapped");
    let out = write_nfo(
        &env,
        &[
            mapped.to_str().expect("utf-8"),
            "--aid",
            "5",
            "--tmdbid",
            "56",
        ],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let nfo = fs::read_to_string(mapped.join("tvshow.nfo")).expect("tvshow.nfo");
    assert!(nfo.contains("type=\"tmdb\""), "{nfo}");
}
