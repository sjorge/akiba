use std::cell::Cell;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use anidb_http::TransportError;
use filetime::{set_file_mtime, FileTime};

use crate::config::CacheConfig;

use super::*;

/// Serves `body` and counts the requests. With no body it fails, so a test
/// that should not touch the network sees an error.
struct FakeTransport {
    body: Option<Vec<u8>>,
    calls: Cell<usize>,
}

impl FakeTransport {
    fn serving(body: &str) -> Self {
        Self {
            body: Some(body.as_bytes().to_vec()),
            calls: Cell::new(0),
        }
    }

    fn offline() -> Self {
        Self {
            body: None,
            calls: Cell::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.get()
    }
}

impl Transport for FakeTransport {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
    ) -> std::result::Result<Vec<u8>, TransportError> {
        self.calls.set(self.calls.get() + 1);
        self.body
            .clone()
            .ok_or_else(|| TransportError::Io(format!("unexpected request to {url}")))
    }
}

const LEGACY_TITLES: &str = r#"
[[1]]
title = "Cowboy Bebop"
type = "main"
language = "x-jat"

[[1]]
title = "Cowboy Bebop (EN)"
type = "official"
language = "en"

[[2]]
title = "Narutxyz"
type = "main"
language = "x-jat"

[[3]]
title = "Naruto"
type = "main"
language = "x-jat"

[[4]]
title = "Fate/Zero"
type = "main"
language = "x-jat"
"#;

const DUMP: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<animetitles>
  <anime aid="1">
    <title type="main" xml:lang="x-jat">Cowboy Bebop</title>
    <title type="official" xml:lang="en">Cowboy Bebop (EN)</title>
    <title type="synonym" xml:lang="en">Bebop</title>
  </anime>
  <anime aid="2">
    <title type="main" xml:lang="x-jat">Tom &amp; Jerry</title>
  </anime>
  <anime aid="3">
    <title type="synonym" xml:lang="en">Only a synonym</title>
  </anime>
</animetitles>
"#;

fn config_in(dir: &Path) -> Config {
    Config {
        cache: CacheConfig {
            path: dir.to_path_buf(),
            ..CacheConfig::default()
        },
        ..Config::default()
    }
}

fn write_titles(dir: &Path, text: &str) -> PathBuf {
    let path = titles_file(dir);
    fs::write(&path, text).expect("write titles");
    path
}

fn set_age(path: &Path, age: Duration) {
    let when = SystemTime::now() - age;
    set_file_mtime(path, FileTime::from_system_time(when)).expect("set mtime");
}

fn fuzzy_resolver() -> (tempfile::TempDir, Resolver) {
    let dir = tempfile::tempdir().expect("tempdir");
    write_titles(dir.path(), LEGACY_TITLES);
    let mut resolver = Resolver::new(&config_in(dir.path()));
    resolver
        .refresh_with(&FakeTransport::offline(), false)
        .expect("fresh cache loads");
    (dir, resolver)
}

fn aid(id: Option<AnimeId>) -> Option<i64> {
    id.map(|id| id.anidb)
}

#[test]
fn fresh_cache_resolves_without_the_network() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_titles(dir.path(), LEGACY_TITLES);
    let transport = FakeTransport::offline();
    let mut resolver = Resolver::new(&config_in(dir.path()));
    resolver.refresh_with(&transport, false).expect("refresh");
    assert_eq!(transport.calls(), 0);

    assert_eq!(aid(resolver.id("Whatever [anidb-123]")), Some(123));
    assert_eq!(aid(resolver.id("Cowboy Bebop")), Some(1));
    assert_eq!(aid(resolver.id("Cowboy Bebap")), Some(1));
    assert_eq!(aid(resolver.id("Narutx")), Some(3));
    assert_eq!(aid(resolver.id("Fate⁄Zero")), Some(4));
    assert_eq!(resolver.id("Completely Different"), None);

    let titles: Vec<&str> = resolver
        .titles(&anidb_only(1))
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(titles, ["Cowboy Bebop", "Cowboy Bebop (EN)"]);
    assert_eq!(resolver.titles(&anidb_only(999)), &[][..]);
}

#[test]
fn every_fraction_slash_is_replaced() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_titles(
        dir.path(),
        "[[7]]\ntitle = \"Fate/Zero/Extra\"\ntype = \"main\"\nlanguage = \"x-jat\"\n",
    );
    let mut resolver = Resolver::new(&config_in(dir.path()));
    resolver
        .refresh_with(&FakeTransport::offline(), false)
        .expect("fresh cache loads");
    assert_eq!(aid(resolver.id("Fate/Zero/Extra")), Some(7));
    assert_eq!(aid(resolver.id("Fate\u{2044}Zero\u{2044}Extra")), Some(7));
}

#[test]
fn exact_title_match_is_found() {
    let (_dir, resolver) = fuzzy_resolver();
    assert_eq!(aid(resolver.id("Naruto")), Some(3));
}

#[test]
fn equal_fuzzy_distances_go_to_the_lowest_aid() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Both titles are one edit from "Alpha Betz". Written out of aid order
    // on purpose, since the map is keyed by aid.
    write_titles(
        dir.path(),
        "[[6]]\ntitle = \"Alpha Betx\"\ntype = \"main\"\nlanguage = \"x-jat\"\n\n\
             [[5]]\ntitle = \"Alpha Beta\"\ntype = \"main\"\nlanguage = \"x-jat\"\n",
    );
    let mut resolver = Resolver::new(&config_in(dir.path()));
    resolver
        .refresh_with(&FakeTransport::offline(), false)
        .expect("fresh cache loads");
    assert_eq!(aid(resolver.id("Alpha Betz")), Some(5));
}

#[test]
fn id_tag_wins_and_overflowing_tag_is_not_a_match() {
    let (_dir, resolver) = fuzzy_resolver();
    assert_eq!(aid(resolver.id("x [anidb-4]")), Some(4));
    // Too large for i64: the tag is ignored and the fuzzy match finds nothing.
    assert_eq!(resolver.id("[anidb-99999999999999999999]"), None);
}

#[test]
fn stale_cache_is_refetched_and_rewritten() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_titles(dir.path(), LEGACY_TITLES);
    set_age(&path, Duration::from_hours(240));

    let transport = FakeTransport::serving(DUMP);
    let mut resolver = Resolver::new(&config_in(dir.path()));
    resolver.refresh_with(&transport, false).expect("refresh");
    assert_eq!(transport.calls(), 1);

    assert_eq!(aid(resolver.id("Tom & Jerry")), Some(2));
    assert_eq!(aid(resolver.id("Bebop")), None);
    assert_eq!(aid(resolver.id("Cowboy Bebop")), Some(1));
    // An aid with no official or main title stays in the cache.
    assert_eq!(resolver.titles(&anidb_only(3)), &[][..]);

    let stored = CacheFile::<TitlesFile>::new(&path, None)
        .load_any_age()
        .expect("load")
        .expect("written");
    assert!(stored.0.contains_key(&3));
    assert_eq!(stored.0[&1].len(), 2);
}

#[test]
fn force_refetches_a_fresh_cache() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_titles(dir.path(), LEGACY_TITLES);
    let transport = FakeTransport::serving(DUMP);
    let mut resolver = Resolver::new(&config_in(dir.path()));
    resolver.refresh_with(&transport, true).expect("refresh");
    assert_eq!(transport.calls(), 1);
    assert_eq!(aid(resolver.id("Tom & Jerry")), Some(2));
}

#[test]
fn missing_cache_is_downloaded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let transport = FakeTransport::serving(DUMP);
    let mut resolver = Resolver::new(&config_in(dir.path()));
    resolver.refresh_with(&transport, false).expect("refresh");
    assert_eq!(transport.calls(), 1);
    assert!(titles_file(dir.path()).is_file());
}

#[test]
fn fixture_titles_cache_loads_without_the_network() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cache/titles.map.toml");
    fs::copy(fixture, titles_file(dir.path())).expect("copy fixture");
    let transport = FakeTransport::offline();
    let mut resolver = Resolver::new(&config_in(dir.path()));
    resolver.refresh_with(&transport, false).expect("refresh");
    assert_eq!(transport.calls(), 0);
    assert_eq!(aid(resolver.id("テスト")), Some(100));
    assert_eq!(resolver.titles(&anidb_only(100)).len(), 3);
}

fn episode_row(file: &EpisodeFile) -> (String, String, String) {
    (
        file.episode_start.clone(),
        file.episode_end.clone(),
        file.title.clone(),
    )
}

#[test]
fn episodes_parses_a_directory_of_episode_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    for file in [
        "Show - 01 - First.mkv",
        "Show - 2 - Second (DEADBEEF).mkv",
        "Show - 10 - Tenth.mp4",
        "Show - S1 - Special.mkv",
        "Show - C2 - Credits.avi",
        "Show - 01-03 - Multi.mkv",
        "Show - 04 - Not Video.txt",
        "Show - 0010 - Padded.ogm",
        "Show - 00 - Zero.mkv",
        "Show - S1-S3 - Range.mkv",
        "Show - 05 - Upper.MKV",
        "garbage.mkv",
    ] {
        fs::write(dir.path().join(file), "").expect("write episode");
    }

    // Natural order, leading zeros stripped, specials kept, checksum removed,
    // and non video or unparseable names skipped.
    let episodes = Resolver::episodes(dir.path()).expect("episodes");
    let rows: Vec<_> = episodes.iter().map(episode_row).collect();
    let row =
        |start: &str, end: &str, title: &str| (start.to_owned(), end.to_owned(), title.to_owned());
    assert_eq!(
        rows,
        [
            row("0", "0", "Zero"),
            row("1", "1", "First"),
            row("1", "3", "Multi"),
            row("2", "2", "Second"),
            row("10", "10", "Padded"),
            row("10", "10", "Tenth"),
            row("C2", "C2", "Credits"),
            row("S1", "S1", "Special"),
            // A range with prefixes is kept as written.
            row("S1", "S3", "Range"),
        ]
    );
    let first = episodes.iter().find(|e| e.title == "First").expect("First");
    assert_eq!(first.path, dir.path().join("Show - 01 - First.mkv"));

    let missing = Resolver::episodes(&dir.path().join("missing")).expect("missing");
    assert_eq!(missing, Vec::new());
    let file = Resolver::episodes(&dir.path().join("garbage.mkv")).expect("file");
    assert_eq!(file, Vec::new());
}

#[test]
fn only_the_last_checksum_is_stripped_from_the_title() {
    let (title, numbers) =
        parse_episode_name("Show - 03 - A (ABCDEF12) (1234ABCD).mkv").expect("parses");
    assert_eq!(numbers, ["3"]);
    assert_eq!(title, "A (ABCDEF12)");
}

fn parsed(name: &str) -> Option<(String, Vec<String>)> {
    parse_episode_name(name)
}

fn episode(title: &str, numbers: &[&str]) -> (String, Vec<String>) {
    let numbers = numbers.iter().map(|&n| n.to_owned()).collect();
    (title.to_owned(), numbers)
}

#[test]
fn single_episode_names_keep_dashes_and_brackets_in_the_title() {
    assert_eq!(
        parsed("Show - 01 - Title (0ba1abec).mkv"),
        Some(episode("Title", &["1"]))
    );
    // The first ` - <episode> - ` wins, the title may contain ` - `.
    assert_eq!(
        parsed("Show - 03 - Part 1 - The Return.mkv"),
        Some(episode("Part 1 - The Return", &["3"]))
    );
    // A part of the show name that is not an episode is skipped.
    assert_eq!(
        parsed("Show - Part 2 - 05 - Title.mkv"),
        Some(episode("Title", &["5"]))
    );
    assert_eq!(
        parsed("[Group] Show (2013) - 07 - The [Big] Day (Part 1).mkv"),
        Some(episode("The [Big] Day (Part 1)", &["7"]))
    );
    assert_eq!(parsed("Show-01-Title.mkv"), None);
    assert_eq!(parsed("Show - 01 - .mkv"), None);
    assert_eq!(parsed("Show - 01 - Title"), None);
    // A four digit episode number is just a number.
    assert_eq!(
        parsed("Show - 2013 - Title.mkv"),
        Some(episode("Title", &["2013"]))
    );
}

#[test]
fn specials_keep_their_prefix() {
    for prefix in ["S", "C", "T", "P", "O", "E"] {
        assert_eq!(
            parsed(&format!("Show - {prefix}01 - Extra.mkv")),
            Some(episode("Extra", &[&format!("{prefix}01")])),
            "{prefix}"
        );
    }
    assert_eq!(parsed("Show - X1 - Extra.mkv"), None);
}

#[test]
fn episode_ranges_give_the_first_and_last_number() {
    assert_eq!(
        parsed("Show - 01-02 - Double.mkv"),
        Some(episode("Double", &["1", "2"]))
    );
    assert_eq!(
        parsed("Show - 09-010 - Double (DEADBEEF).mp4"),
        Some(episode("Double", &["9", "10"]))
    );
    // Prefixed ends are kept as written, and `E` is not a range prefix.
    assert_eq!(
        parsed("Show - S1-S3 - Specials.mkv"),
        Some(episode("Specials", &["S1", "S3"]))
    );
    assert_eq!(parsed("Show - E1-E2 - Extra.mkv"), None);
    assert_eq!(parsed("Show - 01-02-03 - Triple.mkv"), None);
}

#[test]
fn checksums_are_eight_letters_or_digits_in_brackets() {
    assert_eq!(strip_checksum("Title"), "Title");
    assert_eq!(strip_checksum("Title (0ba1abec)"), "Title ");
    assert_eq!(strip_checksum("Title (ZZZZZZZZ)"), "Title ");
    assert_eq!(strip_checksum("Title (0ba1abe)"), "Title (0ba1abe)");
    assert_eq!(strip_checksum("Title (0ba1abec0)"), "Title (0ba1abec0)");
    assert_eq!(strip_checksum("Title (0ba1-bec)"), "Title (0ba1-bec)");
    assert_eq!(strip_checksum("Title (0ba1abec"), "Title (0ba1abec");
    assert_eq!(
        strip_checksum("Title (日本語です日本語です)"),
        "Title (日本語です日本語です)"
    );
    // Not only at the end, but never at the very start.
    assert_eq!(strip_checksum("Title (0ba1abec) [v2]"), "Title  [v2]");
    assert_eq!(strip_checksum("(0ba1abec)"), "(0ba1abec)");
    assert_eq!(strip_checksum("(0ba1abec) Title"), "(0ba1abec) Title");
    // The last checksum is found, its first occurrence removed.
    assert_eq!(
        strip_checksum("(0ba1abec) Title (0ba1abec)"),
        " Title (0ba1abec)"
    );
    assert_eq!(
        parsed("Show - 01 - (0ba1abec).mkv"),
        Some(episode("(0ba1abec)", &["1"]))
    );
}

#[test]
fn the_first_valid_anidb_tag_gives_the_aid() {
    assert_eq!(tagged_aid("Show [anidb-123]"), Some(123));
    assert_eq!(tagged_aid("[anidb-7] Show"), Some(7));
    assert_eq!(
        tagged_aid("[anidb-] [anidb-1x] [anidb-8] [anidb-9]"),
        Some(8)
    );
    assert_eq!(tagged_aid("Show [anidb-12"), None);
    assert_eq!(tagged_aid("Show (anidb-12)"), None);
    assert_eq!(tagged_aid("Show [AniDB-12]"), None);
    assert_eq!(tagged_aid("Show [anidb-١٢]"), None);
}

/// Whitespace is Unicode `White_Space`, as in `char::is_whitespace`:
/// U+0085 (NEL) separates and is trimmed, U+FEFF (BOM) is not whitespace.
/// A carriage return may appear in a title.
#[test]
fn whitespace_follows_the_rust_definition() {
    assert_eq!(
        parsed("Show -\u{85}01\u{85}- Title\u{85}.mkv"),
        Some(episode("Title", &["1"]))
    );
    assert_eq!(parsed("Show -\u{FEFF}01 - Title.mkv"), None);
    assert_eq!(
        parsed("Show - 01 - \u{FEFF}Title.mkv"),
        Some(episode("\u{FEFF}Title", &["1"]))
    );
    assert_eq!(
        parsed("Show -\u{3000}01\u{A0}- Title\u{3000}.mkv"),
        Some(episode("Title", &["1"]))
    );
    assert_eq!(
        parsed("Show - 01 - A\rB.mkv"),
        Some(episode("A\rB", &["1"]))
    );
}

#[test]
fn prefixed_ranges_keep_their_prefix_on_both_ends() {
    for prefix in ['S', 'C', 'T', 'P', 'O'] {
        for (first, last) in [("1", "3"), ("01", "03"), ("001", "12")] {
            let name = format!("Show - {prefix}{first}-{prefix}{last} - Title.mkv");
            assert_eq!(
                parsed(&name),
                Some(episode(
                    "Title",
                    &[&format!("{prefix}{first}"), &format!("{prefix}{last}")]
                )),
                "{name}"
            );
        }
    }
    // a mixed range keeps the prefixed end and normalises the plain one
    assert_eq!(
        parsed("Show - S01-03 - Title.mkv"),
        Some(episode("Title", &["S01", "3"]))
    );
    // nothing ever turns into the text NaN
    assert!(!format!("{:?}", parsed("Show - C1-C2 - Title.mkv")).contains("NaN"));
}

#[test]
fn single_numbers_lose_zeros_only_without_a_prefix() {
    for (text, expected) in [("1", "1"), ("01", "1"), ("000", "0"), ("0010", "10")] {
        assert_eq!(
            parsed(&format!("Show - {text} - Title.mkv")),
            Some(episode("Title", &[expected])),
            "{text}"
        );
    }
    for prefix in ['S', 'C', 'T', 'P', 'O', 'E'] {
        for digits in ["1", "01", "001"] {
            assert_eq!(
                parsed(&format!("Show - {prefix}{digits} - Title.mkv")),
                Some(episode("Title", &[&format!("{prefix}{digits}")])),
                "{prefix}{digits}"
            );
        }
    }
}
