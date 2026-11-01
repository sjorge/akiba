use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use filetime::{set_file_mtime, FileTime};

use super::paths;
use super::records::{HashesFile, ListFile, LocalFile, TitlesFile};
use super::*;
use crate::anime::TitleVariant;
use crate::fileinfo::FileInfo;
use anidb_http::AniDbShow;

const FIXTURE_CACHE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cache");

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read fixture dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn fixture_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(Path::new(FIXTURE_CACHE), &mut files);
    files.sort();
    files
}

fn set_age(path: &Path, age: Duration) {
    let when = SystemTime::now() - age;
    set_file_mtime(path, FileTime::from_system_time(when)).expect("set mtime");
}

fn sample_titles() -> TitlesFile {
    KeyedMap(BTreeMap::from([
        (
            1,
            vec![TitleVariant {
                title: "Anime One".into(),
                kind: Some("main".into()),
                year: None,
                language: "x-jat".into(),
            }],
        ),
        (100, Vec::new()),
    ]))
}

/// Loads a fixture file as `T`, stores it into `dir` and loads it back.
/// Returns the fixture value as JSON so callers can compare it.
fn round_trip<T>(fixture: &Path, dir: &Path) -> serde_json::Value
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let loaded = CacheFile::<T>::new(fixture, None)
        .load_any_age()
        .expect("fixture loads")
        .expect("fixture exists");
    let target = dir.join(fixture.file_name().expect("name"));
    let cache = CacheFile::<T>::new(&target, None);
    cache.store(&loaded).expect("store");
    let reloaded = cache.load().expect("reload").expect("stored file exists");
    assert_eq!(reloaded, loaded, "{}", fixture.display());
    serde_json::to_value(&loaded).expect("json")
}

#[test]
fn every_fixture_cache_file_loads_into_its_type() {
    let dir = tempfile::tempdir().expect("tempdir");
    let files = fixture_files();
    assert_eq!(files.len(), 6, "unexpected fixture cache files: {files:?}");

    for file in &files {
        let name = file.file_name().and_then(|n| n.to_str()).expect("name");
        let parent = file.parent().and_then(|p| p.file_name());
        if name.ends_with(".aid.toml") {
            round_trip::<AniDbShow>(file, dir.path());
        } else if name.ends_with(".fid.toml") {
            round_trip::<FileInfo>(file, dir.path());
        } else if name.ends_with(".hashes.toml") {
            round_trip::<HashesFile>(file, dir.path());
        } else if name == "titles.map.toml" {
            round_trip::<TitlesFile>(file, dir.path());
        } else if name == "list.map.toml" {
            round_trip::<ListFile>(file, dir.path());
        } else if name == "local.map.toml" {
            round_trip::<LocalFile>(file, dir.path());
        } else {
            panic!(
                "no cache type for fixture file {} ({parent:?})",
                file.display()
            );
        }
    }
}

#[test]
fn fixture_local_map_keeps_the_blacklist_markers() {
    let file = Path::new(FIXTURE_CACHE).join("local.map.toml");
    let local = CacheFile::<LocalFile>::new(&file, None)
        .load_any_age()
        .expect("load")
        .expect("present");
    let entry = &local.0[&200];
    assert_eq!(entry.anilist, Some(-1));
    assert_eq!(entry.tmdb, Some(-1));
    assert_eq!(local.0[&10914].tmdb_season, Some(1));
}

#[test]
fn invalid_keys_are_parse_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("list.map.toml");
    fs::write(&path, "[abc]\nanilist = 1\n").expect("write");
    let err = CacheFile::<ListFile>::new(&path, None)
        .load_any_age()
        .expect_err("bad key");
    assert!(matches!(err, CacheError::Parse { .. }), "{err:?}");
}

#[test]
fn store_creates_missing_parent_directories() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("anidb").join("100.aid.toml");
    let cache = CacheFile::<AniDbShow>::new(&path, None);
    cache.store(&AniDbShow::default()).expect("store");
    assert!(path.is_file());
}

#[test]
fn missing_file_loads_as_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = CacheFile::<ListFile>::new(dir.path().join("nope.toml"), Some(1));
    assert!(!cache.is_fresh());
    assert_eq!(cache.load().expect("load"), None);
    assert_eq!(cache.load_any_age().expect("load"), None);
}

#[test]
fn freshness_follows_fractional_days() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("titles.map.toml");
    let cache = CacheFile::<TitlesFile>::new(&path, Some(1));
    cache.store(&sample_titles()).expect("store");

    set_age(&path, Duration::from_hours(23));
    assert!(cache.is_fresh());
    assert!(cache.load().expect("load").is_some());

    set_age(&path, Duration::from_hours(25));
    assert!(!cache.is_fresh());
    assert_eq!(cache.load().expect("load"), None);
    // load_any_age ignores the age
    assert!(cache.load_any_age().expect("load").is_some());
}

#[test]
fn zero_day_limit_is_always_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("titles.map.toml");
    let cache = CacheFile::<TitlesFile>::new(&path, Some(0));
    cache.store(&sample_titles()).expect("store");
    assert!(!cache.is_fresh());
}

#[test]
fn no_limit_never_expires() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("local.map.toml");
    let cache = CacheFile::<LocalFile>::new(&path, None);
    cache.store(&KeyedMap::default()).expect("store");
    set_age(&path, Duration::from_hours(10 * 365 * 24));
    assert!(cache.is_fresh());
}

#[test]
fn future_mtime_counts_as_fresh() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("titles.map.toml");
    let cache = CacheFile::<TitlesFile>::new(&path, Some(1));
    cache.store(&sample_titles()).expect("store");
    let future = SystemTime::now() + Duration::from_secs(3600);
    set_file_mtime(&path, FileTime::from_system_time(future)).expect("set mtime");
    assert!(cache.is_fresh());
}

#[test]
fn store_then_load_round_trips_every_type() {
    let dir = tempfile::tempdir().expect("tempdir");

    let titles = CacheFile::<TitlesFile>::new(dir.path().join("t.toml"), Some(3));
    titles.store(&sample_titles()).expect("store");
    assert_eq!(titles.load().expect("load"), Some(sample_titles()));

    let hashes = BTreeMap::from([(
        "/library/a.mkv".to_owned(),
        records::HashRecord {
            hash: "7dafd31d6d028e981d09301c5fbfc654".into(),
            size: 447_945_964,
            link: "ed2k://|file|a.mkv|447945964|7dafd31d6d028e981d09301c5fbfc654|/".into(),
            created_at: 1_767_225_600_000,
        },
    )]);
    let hashes_cache = CacheFile::<HashesFile>::new(dir.path().join("h.toml"), None);
    hashes_cache.store(&hashes).expect("store");
    assert_eq!(hashes_cache.load().expect("load"), Some(hashes));

    let info = FileInfo {
        fid: 1_628_637,
        anime_year: vec![2015, 2015],
        anime_synonyms: vec!["x".into()],
        quality: "very high".into(),
        ..FileInfo::default()
    };
    let info_cache = CacheFile::<FileInfo>::new(dir.path().join("f.toml"), None);
    info_cache.store(&info).expect("store");
    assert_eq!(info_cache.load().expect("load"), Some(info));
}

#[test]
fn hashes_file_path_uses_the_hostname_hash() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = paths::hashes_file(dir.path()).expect("path");
    assert_eq!(path.parent(), Some(dir.path()));
    assert!(path.to_string_lossy().ends_with(".hashes.toml"));
}
