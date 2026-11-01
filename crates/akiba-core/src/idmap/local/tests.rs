use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::cache::CacheError;
use crate::idmap::IdMapError;

fn config_in(dir: &Path) -> Config {
    let mut config = Config::default();
    config.cache.path = dir.to_path_buf();
    config
}

fn fixture_cache() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cache")
}

fn anime(anidb: i64) -> AnimeId {
    AnimeId {
        anidb,
        anilist: None,
        tvdb: None,
        tvdb_season: None,
        tmdb: None,
        tmdb_season: None,
    }
}

fn write_local(dir: &Path, text: &str) {
    fs::write(dir.join("local.map.toml"), text).expect("write local map");
}

#[test]
fn missing_file_is_not_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut map = LocalMap::new(&config_in(dir.path()));
    assert!(!map.refresh().expect("refresh"));
    assert_eq!(map.entry(1), None);
}

#[test]
fn empty_file_is_an_empty_mapping() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_local(dir.path(), "");
    let mut map = LocalMap::new(&config_in(dir.path()));
    map.refresh().expect("an empty file is not an error");
    assert_eq!(map.entry(1), None);
    // nothing to remove, so unsetting is a no-op that leaves the desired state
    assert!(!map.unset_anilist(1));
    assert!(!map.unset_tmdb(1));
}

#[test]
fn first_mapping_creates_the_file_and_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("not").join("there").join("yet");
    let mut map = LocalMap::new(&config_in(&cache));
    assert!(!map.refresh().expect("missing file"));
    map.set_anilist(7, 42);
    map.save().expect("save creates the file");
    let mut reloaded = LocalMap::new(&config_in(&cache));
    assert!(reloaded.refresh().expect("reload"));
    assert_eq!(reloaded.entry(7).and_then(|e| e.anilist), Some(42));
}

#[test]
fn invalid_file_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_local(dir.path(), "[[[");
    let mut map = LocalMap::new(&config_in(dir.path()));
    let err = map.refresh().expect_err("invalid file");
    assert!(
        matches!(err, IdMapError::Cache(CacheError::Parse { .. })),
        "{err:?}"
    );
}

#[test]
fn refresh_applies_the_mapping_without_overwriting() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_local(dir.path(), "[1]\nanilist = 10\ntmdb = 20\ntmdbSeason = 2\n");
    let mut map = LocalMap::new(&config_in(dir.path()));
    assert!(map.refresh().expect("refresh"));

    let mut empty = anime(1);
    map.apply(&mut empty, &[], false).expect("apply");
    assert_eq!(
        empty,
        AnimeId {
            anilist: Some(10),
            tmdb: Some(20),
            tmdb_season: Some(2),
            ..anime(1)
        }
    );

    let mut existing = AnimeId {
        anilist: Some(99),
        tmdb: Some(98),
        ..anime(1)
    };
    map.apply(&mut existing, &[], false).expect("apply");
    assert_eq!(
        existing,
        AnimeId {
            anilist: Some(99),
            tmdb: Some(98),
            ..anime(1)
        }
    );

    map.apply(&mut existing, &[], true).expect("apply");
    assert_eq!(
        existing,
        AnimeId {
            anilist: Some(10),
            tmdb: Some(20),
            tmdb_season: Some(2),
            ..anime(1)
        }
    );

    let mut unknown = anime(2);
    map.apply(&mut unknown, &[], false).expect("apply");
    assert_eq!(unknown, anime(2));
}

#[test]
fn overwrite_copies_a_missing_season() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_local(dir.path(), "[1]\ntmdb = 20\n");
    let mut map = LocalMap::new(&config_in(dir.path()));
    map.refresh().expect("refresh");

    let mut id = AnimeId {
        tmdb: Some(5),
        tmdb_season: Some(3),
        ..anime(1)
    };
    map.apply(&mut id, &[], true).expect("apply");
    assert_eq!(id.tmdb, Some(20));
    assert_eq!(id.tmdb_season, None);
}

#[test]
fn zero_is_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_local(dir.path(), "[1]\nanilist = 0\ntmdb = 0\n");
    let mut map = LocalMap::new(&config_in(dir.path()));
    map.refresh().expect("refresh");

    let mut id = anime(1);
    map.apply(&mut id, &[], true).expect("apply");
    assert_eq!(id, anime(1));
}

#[test]
fn fixture_blacklist_markers_apply_and_block_lookups() {
    let config = config_in(&fixture_cache());
    let mut map = LocalMap::new(&config);
    assert!(map.refresh().expect("refresh"));

    let mut id = anime(200);
    map.apply(&mut id, &[], false).expect("apply");
    assert_eq!(id.anilist, Some(-1));
    assert_eq!(id.tmdb, Some(-1));
    assert_eq!(id.tmdb_season, None);
}

#[test]
fn set_and_unset_follow_the_idmap_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = config_in(dir.path());
    let mut map = LocalMap::new(&config);

    map.set_anilist(5, -1);
    map.set_tmdb(5, 9);
    assert_eq!(map.entry(5).expect("entry").tmdb_season, Some(1));
    map.save().expect("save");

    let mut reloaded = LocalMap::new(&config);
    assert!(reloaded.refresh().expect("refresh"));
    let entry = reloaded.entry(5).expect("saved entry");
    assert_eq!(entry.anilist, Some(-1));
    assert_eq!(entry.tmdb, Some(9));
    assert_eq!(entry.tmdb_season, Some(1));

    assert!(reloaded.unset_tmdb(5));
    assert_eq!(reloaded.entry(5).expect("kept").tmdb, None);
    assert_eq!(reloaded.entry(5).expect("kept").tmdb_season, None);
    assert!(!reloaded.unset_tmdb(5));

    assert!(reloaded.unset_anilist(5));
    assert_eq!(reloaded.entry(5), None, "an empty entry is dropped");
    assert!(!reloaded.unset_anilist(5));
}

#[test]
fn unset_keeps_the_other_provider() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut map = LocalMap::new(&config_in(dir.path()));
    map.set_anilist(7, 70);
    map.set_tmdb(7, 77);

    assert!(map.unset_anilist(7));
    let entry = map.entry(7).expect("tmdb mapping kept");
    assert_eq!(entry.anilist, None);
    assert_eq!(entry.tmdb, Some(77));
    assert_eq!(entry.tmdb_season, Some(1));
}

#[test]
fn blacklist_round_trips_through_save() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = config_in(dir.path());
    let mut map = LocalMap::new(&config);
    map.set_tmdb(200, -1);
    map.set_anilist(200, -1);
    map.save().expect("save");

    let text = fs::read_to_string(dir.path().join("local.map.toml")).expect("read");
    assert!(text.contains("anilist = -1"), "{text}");
    assert!(text.contains("tmdb = -1"), "{text}");
}
