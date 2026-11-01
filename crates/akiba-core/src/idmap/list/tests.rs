use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use filetime::{set_file_mtime, FileTime};
use serde_json::json;

use super::*;
use crate::cache::CacheError;
use crate::http::fake::{json as json_reply, FakeHttp};
use crate::idmap::IdMapError;

fn config_in(dir: &Path, mapping_age: u32) -> Config {
    let mut config = Config::default();
    config.cache.path = dir.to_path_buf();
    config.cache.mapping_age = mapping_age;
    config
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

fn seed(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("list.map.toml");
    fs::write(&path, text).expect("seed list map");
    path
}

fn age_file(path: &Path, days: u64) {
    let when = SystemTime::now() - Duration::from_secs(days * 86_400);
    set_file_mtime(path, FileTime::from_system_time(when)).expect("set mtime");
}

#[test]
fn fresh_cache_loads_without_the_network() {
    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path(), "[1]\nanilist = 10\n");
    let config = config_in(dir.path(), 7);
    let mut list = ListMap::new(&config);
    let http = FakeHttp::offline();
    list.refresh(&http).expect("refresh");
    assert_eq!(http.calls(), []);

    let mut empty = anime(1);
    list.apply(&mut empty, &[], false).expect("apply");
    assert_eq!(
        empty,
        AnimeId {
            anilist: Some(10),
            ..anime(1)
        }
    );

    let mut existing = AnimeId {
        anilist: Some(99),
        ..anime(1)
    };
    list.apply(&mut existing, &[], false).expect("apply");
    assert_eq!(existing.anilist, Some(99));
    list.apply(&mut existing, &[], true).expect("apply");
    assert_eq!(existing.anilist, Some(10));

    let mut unknown = anime(2);
    list.apply(&mut unknown, &[], false).expect("apply");
    assert_eq!(unknown, anime(2));
}

#[test]
fn corrupt_fresh_cache_is_an_error_and_is_not_refetched() {
    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path(), "[[[");
    let mut list = ListMap::new(&config_in(dir.path(), 7));
    let http = FakeHttp::new(|_| panic!("no download expected"));
    let err = list.refresh(&http).expect_err("corrupt cache");
    assert!(
        matches!(err, IdMapError::Cache(CacheError::Parse { .. })),
        "{err:?}"
    );
    assert_eq!(http.calls(), []);
}

#[test]
fn stale_cache_is_downloaded_and_rewritten() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = seed(dir.path(), "[1]\nanilist = 1\n");
    age_file(&path, 8);

    let reply = json!([
        {"anidb_id": 1, "anilist_id": 10, "type": "TV"},
        {"anidb_id": 2, "anilist_id": 0},
        {"anidb_id": 3},
        {"anilist_id": 30},
        {"anidb_id": 1, "anilist_id": 11},
        {"anidb_id": 4, "anilist_id": 40, "kitsu_id": null},
    ]);
    let http = FakeHttp::new(move |call| {
        assert_eq!(call.url, LIST_URL);
        json_reply(&reply)
    });

    let config = config_in(dir.path(), 7);
    let mut list = ListMap::new(&config);
    list.refresh(&http).expect("refresh");
    assert_eq!(http.calls().len(), 1);
    assert_eq!(list.anilist(1), Some(11), "last entry wins");
    assert_eq!(list.anilist(2), None, "zero is not a mapping");
    assert_eq!(list.anilist(3), None);
    assert_eq!(list.anilist(4), Some(40));

    // The rewritten cache is fresh, so a second load makes no call.
    let mut reloaded = ListMap::new(&config);
    let offline = FakeHttp::offline();
    reloaded.refresh(&offline).expect("fresh cache");
    assert_eq!(offline.calls(), []);
    assert_eq!(reloaded.anilist(4), Some(40));
}

#[test]
fn missing_cache_is_downloaded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let http = FakeHttp::new(|_| json_reply(&json!([{"anidb_id": 9, "anilist_id": 90}])));
    let mut list = ListMap::new(&config_in(dir.path(), 7));
    list.refresh(&http).expect("refresh");
    assert_eq!(list.anilist(9), Some(90));
}

#[test]
fn failed_download_leaves_the_mapping_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = seed(dir.path(), "[1]\nanilist = 1\n");
    age_file(&path, 8);

    let http = FakeHttp::new(|call| Err(HttpError::new(&call.url, "HTTP status 503")));
    let mut list = ListMap::new(&config_in(dir.path(), 7));
    let err = list.refresh(&http).expect_err("download fails");
    assert!(matches!(err, IdMapError::Http(_)), "{err:?}");
    assert_eq!(list.anilist(1), None, "a stale cache is not used");

    let mut id = anime(1);
    list.apply(&mut id, &[], true).expect("apply");
    assert_eq!(id, anime(1));
}

#[test]
fn a_reply_that_is_not_an_array_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let http = FakeHttp::new(|_| json_reply(&json!({"oops": true})));
    let mut list = ListMap::new(&config_in(dir.path(), 7));
    let err = list.refresh(&http).expect_err("not an array");
    assert!(matches!(err, IdMapError::Http(_)), "{err:?}");
}
