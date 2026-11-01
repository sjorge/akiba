use anidb_http::Title;
use serde_json::Value;

use super::*;
use crate::nfo::testing::{events, fixture};
use crate::nfo::{EpisodeNfo, TvShowNfo};

fn anime_id(anidb: i64) -> AnimeId {
    AnimeId {
        anidb,
        anilist: None,
        tvdb: None,
        tvdb_season: None,
        tmdb: None,
        tmdb_season: None,
    }
}

fn title(text: &str, kind: Option<&str>, language: &str) -> Title {
    Title {
        title: text.to_string(),
        kind: kind.map(str::to_string),
        language: language.to_string(),
    }
}

fn anidb_episode(id: i64, number: &str, kind: i64, air_date: &str) -> AniDbEpisode {
    AniDbEpisode {
        id,
        episode_number: number.to_string(),
        kind,
        air_date: air_date.to_string(),
        titles: vec![title("Ep", None, "en")],
        ..AniDbEpisode::default()
    }
}

/// A fully filled show.
fn make_show() -> AniDbShow {
    AniDbShow {
        id: 100,
        age_restricted: false,
        kind: "TV Series".to_string(),
        episode_count: 3,
        start_date: "2020-01".to_string(),
        end_date: "2020-03-25".to_string(),
        titles: vec![
            title("Test `Anime` EN", Some("official"), "en"),
            title("Test `Anime`", Some("main"), "x-jat"),
            title("テスト", Some("official"), "ja"),
        ],
        description: "A plot".to_string(),
        picture: "12345.jpg".to_string(),
        url: "https://anidb.net/anime/100".to_string(),
        episodes: vec![
            AniDbEpisode {
                titles: vec![title("First `Ep`", None, "en"), title("第一話", None, "ja")],
                air_date: "2020-01-08".to_string(),
                ..anidb_episode(1001, "1", 1, "2020-01-08")
            },
            AniDbEpisode {
                titles: vec![title("Second", None, "en")],
                ..anidb_episode(1002, "2", 1, "2020-01")
            },
            AniDbEpisode {
                titles: vec![title("Third", None, "en")],
                ..anidb_episode(1003, "3", 1, "")
            },
            AniDbEpisode {
                titles: vec![title("Special", None, "en")],
                ..anidb_episode(2001, "S1", 2, "2020-04-01")
            },
        ],
        ..AniDbShow::default()
    }
}

fn file(start: &'static str, end: &'static str) -> EpisodeFileInfo<'static> {
    EpisodeFileInfo {
        episode_start: start,
        episode_end: end,
        title: "File `Title`",
    }
}

#[test]
fn tv_show_maps_anidb_metadata() {
    let show = tv_show(&anime_id(100), &make_show()).expect("titles");

    // NOTE: regression for replaceAll, both backticks must be replaced
    assert_eq!(show.title, "Test 'Anime'");
    assert_eq!(show.original_title.as_deref(), Some("テスト"));
    assert_eq!(
        show.unique_ids,
        vec![UniqueId {
            id: "100".to_string(),
            kind: "anidb".to_string(),
            default: true,
        }]
    );
    assert_eq!(show.premiered.as_deref(), Some("2020-01-01"));
    assert_eq!(show.mpaa.as_deref(), Some("G"));
    assert_eq!(show.plot, None, "the builder does not map the description");

    let dir = tempfile::tempdir().expect("tempdir");
    let mut nfo = TvShowNfo::new(show, dir.path()).expect("dir");
    assert!(nfo.is_valid());
}

#[test]
fn tv_show_without_titles_is_an_error() {
    let mut metadata = make_show();
    metadata.titles.clear();
    let err = tv_show(&anime_id(100), &metadata).expect_err("no titles");
    assert!(matches!(err, NfoError::NoTitles { aid: 100 }), "{err:?}");
    assert!(err.to_string().contains("has no titles"), "{err}");
}

#[test]
fn tv_show_ids_skip_zero_and_blacklisted_values() {
    let ids = |anilist, tmdb| {
        tv_show(
            &AnimeId {
                anilist,
                tmdb,
                ..anime_id(100)
            },
            &make_show(),
        )
        .expect("titles")
        .unique_ids
        .into_iter()
        .map(|id| (id.kind, id.id))
        .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(Some(5), Some(6)),
        [("anidb", "100"), ("anilist", "5"), ("tmdb", "6")]
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .to_vec()
    );
    assert_eq!(ids(Some(-1), Some(-1)).len(), 1);
    assert_eq!(ids(Some(0), None).len(), 1);
}

#[test]
fn tv_show_dates_and_rating() {
    let mut metadata = make_show();
    metadata.start_date = "2020-02-03".to_string();
    let id = anime_id(100);
    assert_eq!(
        tv_show(&id, &metadata)
            .expect("titles")
            .premiered
            .as_deref(),
        Some("2020-02-03")
    );

    metadata.start_date = "2020".to_string();
    assert_eq!(tv_show(&id, &metadata).expect("titles").premiered, None);

    metadata.age_restricted = true;
    assert_eq!(
        tv_show(&id, &metadata).expect("titles").mpaa.as_deref(),
        Some("18+")
    );
}

#[test]
fn tv_show_prefers_x_jat_main_title() {
    let mut metadata = make_show();
    metadata.titles.swap(0, 1);
    let show = tv_show(&anime_id(100), &metadata).expect("titles");
    assert_eq!(show.title, "Test 'Anime'");
}

#[test]
fn episodes_map_anidb_episodes() {
    let episodes = episodes(&file("1", "1"), &make_show());
    assert_eq!(episodes.len(), 1);
    let first = &episodes[0];
    assert_eq!(first.season, Some(1));
    assert_eq!(first.episode, Some(1));
    assert_eq!(
        first.unique_ids,
        vec![UniqueId {
            id: "1001".to_string(),
            kind: "anidb".to_string(),
            default: true,
        }]
    );
    // en wins over the file title, ja is the original title
    assert_eq!(first.title, "First 'Ep'");
    assert_eq!(first.original_title.as_deref(), Some("第一話"));
}

#[test]
fn episode_ranges_fall_back_to_file_title_and_pad_airdates() {
    let range = episodes(&file("1", "3"), &make_show());
    assert_eq!(
        range.iter().map(|e| e.episode).collect::<Vec<_>>(),
        [Some(1), Some(2), Some(3)]
    );
    assert_eq!(
        range
            .iter()
            .map(|e| e.premiered.as_deref())
            .collect::<Vec<_>>(),
        [Some("2020-01-08"), Some("2020-01-01"), None]
    );
    assert_eq!(range[2].title, "Third");
}

#[test]
fn specials_are_season_zero_numbered_from_their_type() {
    let special = episodes(&file("S1", "S1"), &make_show());
    assert_eq!(special.len(), 1);
    assert_eq!(special[0].season, Some(0));
    assert_eq!(special[0].episode, Some(201));
}

#[test]
fn episodes_without_metadata_are_empty() {
    assert_eq!(episodes(&file("9", "9"), &make_show()).len(), 0);
    assert_eq!(episodes(&file("S1", "S3"), &make_show()).len(), 0);
}

#[test]
fn expand_matches_brace_fixture_cases() {
    let raw = std::fs::read_to_string(format!(
        "{}/tests/fixtures/brace-ranges.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("brace-ranges.json");
    let cases: Value = serde_json::from_str(&raw).expect("json");
    let cases = cases.as_array().expect("array");
    assert_ne!(cases.len(), 0);
    for case in cases {
        let start = case["start"].as_str().expect("start");
        let end = case["end"].as_str().expect("end");
        let expected: Vec<String> = case["expanded"]
            .as_array()
            .expect("expanded")
            .iter()
            .map(|v| v.as_str().expect("string").to_string())
            .collect();
        assert_eq!(expand_episode_range(start, end), expected, "{start}..{end}");
    }
}

#[test]
fn expand_follows_braces_for_other_inputs() {
    // descending and padded in both directions
    assert_eq!(expand_episode_range("1", "011")[9..], ["010", "011"]);
    assert_eq!(expand_episode_range("-1", "1"), ["-1", "0", "1"]);
    assert_eq!(expand_episode_range("0", "2"), ["0", "1", "2"]);
    assert_eq!(expand_episode_range("1.5", "3"), ["{1.5..3}"]);
    assert_eq!(expand_episode_range("", "3"), ["{..3}"]);
    assert_eq!(expand_episode_range("a", "c"), ["a", "b", "c"]);
    assert_eq!(expand_episode_range("c", "a"), ["c", "b", "a"]);
    assert_eq!(expand_episode_range("ab", "c"), ["{ab..c}"]);
}

#[test]
fn pad_keeps_sign_outside_the_width() {
    assert_eq!(pad(-3, 3), "-03");
    assert_eq!(pad(7, 2), "07");
    assert_eq!(pad(123, 2), "123");
    assert_eq!(pad(-5, 0), "-5");
}

#[test]
fn episode_numbers_map_per_type_and_stay_special() {
    let numbered =
        |number: &str, kind: i64| episode(&file("1", "1"), &anidb_episode(1, number, kind, ""));
    // regular episodes are season 1, plain digits with or without leading zeros
    for (number, expected) in [("1", 1), ("01", 1), ("12", 12), ("0", 0)] {
        let mapped = numbered(number, 1);
        assert_eq!((mapped.season, mapped.episode), (Some(1), Some(expected)));
    }
    // every other type is season 0, numbered from the type: type * 100 + n
    for (prefix, kind) in [('S', 2), ('C', 3), ('T', 4), ('P', 5), ('O', 6)] {
        for (digits, n) in [("1", 1), ("01", 1), ("12", 12)] {
            let mapped = numbered(&format!("{prefix}{digits}"), kind);
            assert_eq!(mapped.season, Some(0), "{prefix}{digits}");
            assert_eq!(mapped.episode, Some(kind * 100 + n), "{prefix}{digits}");
        }
    }
}

#[test]
fn episode_numbers_that_do_not_parse_are_omitted() {
    for (number, kind) in [
        ("1x", 1),
        ("x", 1),
        ("", 1),
        ("1.5", 1),
        ("S1x", 2),
        ("S", 2),
    ] {
        let mapped = episode(&file("1", "1"), &anidb_episode(1, number, kind, ""));
        assert_eq!(mapped.episode, None, "{number:?}");
    }
}

#[test]
fn tv_show_matches_fixture_nfo() {
    let dir = tempfile::tempdir().expect("tempdir");
    let show = tv_show(&anime_id(100), &make_show()).expect("titles");
    TvShowNfo::new(show, dir.path())
        .expect("dir")
        .write(true, None)
        .expect("write");
    let ours = std::fs::read(dir.path().join("tvshow.nfo")).expect("nfo");
    assert_eq!(events(&ours), events(&fixture("tvshow.nfo")));
}

#[test]
fn tv_show_with_ids_matches_fixture_nfo() {
    let dir = tempfile::tempdir().expect("tempdir");
    let id = AnimeId {
        anilist: Some(5),
        tmdb: Some(6),
        ..anime_id(100)
    };
    TvShowNfo::new(tv_show(&id, &make_show()).expect("titles"), dir.path())
        .expect("dir")
        .write(true, None)
        .expect("write");
    let ours = std::fs::read(dir.path().join("tvshow.nfo")).expect("nfo");
    assert_eq!(events(&ours), events(&fixture("tvshow-ids.nfo")));
}

#[test]
fn episode_nfos_match_fixtures() {
    let cases = [
        ("episode-1.nfo", file("1", "1")),
        ("episode-1-3.nfo", file("1", "3")),
        ("episode-S1.nfo", file("S1", "S1")),
    ];
    for (name, info) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let media = dir.path().join(name.replace(".nfo", ".mkv"));
        std::fs::write(&media, "").expect("media");
        let mut nfo = EpisodeNfo::new(episodes(&info, &make_show()), &media).expect("file");
        nfo.write(true).expect("write");
        let ours = std::fs::read(media.with_extension("nfo")).expect("nfo");
        assert_eq!(events(&ours), events(&fixture(name)), "{name}");
    }
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

#[test]
fn modified_time_is_noon_utc_of_the_date() {
    assert_eq!(modified_time("1970-01-01"), Some(at(43_200)));
    assert_eq!(modified_time("2020-01-08"), Some(at(1_578_484_800)));
    assert_eq!(modified_time("2000-03-01"), Some(at(951_912_000)));
    assert_eq!(modified_time("2038-01-19"), Some(at(2_147_515_200)));
}

#[test]
fn modified_time_uses_the_first_day_of_a_partial_date() {
    assert_eq!(modified_time("2020-03"), modified_time("2020-03-01"));
    assert_eq!(modified_time("2020"), modified_time("2020-01-01"));
    assert_eq!(modified_time("2020"), Some(at(1_577_880_000)));
}

#[test]
fn modified_time_knows_leap_years() {
    assert!(modified_time("2020-02-29").is_some());
    assert!(modified_time("2000-02-29").is_some());
    assert_eq!(modified_time("2021-02-29"), None);
    assert_eq!(modified_time("1900-02-29"), None);
    assert_eq!(modified_time("2100-02-29"), None);
    assert_eq!(
        modified_time("2020-03-01").zip(modified_time("2020-02-29")),
        Some(at(1_583_064_000)).zip(Some(at(1_582_977_600)))
    );
}

#[test]
fn modified_time_ignores_missing_malformed_and_early_dates() {
    for date in [
        "",
        "abc",
        "2020-1-5",
        "2020-13-01",
        "2020-00-10",
        "2020-04-31",
        "2020-01-00",
        "2020-01-32",
        "2020-01-01-01",
        "20-01-01",
        "2020-0a-01",
        "+020-01-01",
        " 2020-01-01",
        "2020-01-01 ",
        "1969-12-31",
        "1900",
        "0000-01-01",
    ] {
        assert_eq!(modified_time(date), None, "{date:?}");
    }
}

#[test]
fn builders_fill_the_modification_time() {
    let mut metadata = make_show();
    metadata.start_date = "2020-01".to_string();
    let show = tv_show(&anime_id(100), &metadata).expect("titles");
    assert_eq!(show.modified, modified_time("2020-01-01"));

    metadata.start_date = String::new();
    let show = tv_show(&anime_id(100), &metadata).expect("titles");
    assert_eq!(show.modified, None);

    let list = episodes(&file("1", "3"), &make_show());
    assert_eq!(list[0].modified, modified_time("2020-01-08"));
    assert_eq!(list[1].modified, modified_time("2020-01-01"));
    assert_eq!(list[2].modified, None);
}

fn mtime(path: &std::path::Path) -> SystemTime {
    std::fs::metadata(path)
        .expect("metadata")
        .modified()
        .expect("mtime")
}

#[test]
fn nfos_get_the_anidb_date_as_modification_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    let metadata = make_show();
    let show = tv_show(&anime_id(100), &metadata).expect("titles");
    TvShowNfo::new(show, dir.path())
        .expect("dir")
        .write(false, None)
        .expect("write");
    assert_eq!(
        Some(mtime(&dir.path().join("tvshow.nfo"))),
        modified_time("2020-01-01")
    );

    let media = dir.path().join("Show - 01.mkv");
    std::fs::write(&media, "").expect("media");
    EpisodeNfo::new(episodes(&file("1", "1"), &metadata), &media)
        .expect("file")
        .write(false)
        .expect("write");
    assert_eq!(
        Some(mtime(&media.with_extension("nfo"))),
        modified_time("2020-01-08")
    );
}

#[test]
fn a_range_takes_the_time_of_its_first_episode() {
    let dir = tempfile::tempdir().expect("tempdir");
    let media = dir.path().join("Show - 01-02.mkv");
    std::fs::write(&media, "").expect("media");
    EpisodeNfo::new(episodes(&file("1", "2"), &make_show()), &media)
        .expect("file")
        .write(false)
        .expect("write");
    assert_eq!(
        Some(mtime(&media.with_extension("nfo"))),
        modified_time("2020-01-08")
    );
}

#[test]
fn a_kept_nfo_is_left_alone_content_and_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    let metadata = make_show();
    let media = dir.path().join("Show - 01.mkv");
    std::fs::write(&media, "").expect("media");
    let nfo_path = media.with_extension("nfo");
    std::fs::write(&nfo_path, "old").expect("seed");
    let tv_path = dir.path().join("tvshow.nfo");
    std::fs::write(&tv_path, "old").expect("seed");
    // an NFO from an older run: only `--force` brings its time in line
    let old_time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    for path in [&nfo_path, &tv_path] {
        std::fs::File::open(path)
            .expect("open")
            .set_modified(old_time)
            .expect("old time");
    }

    EpisodeNfo::new(episodes(&file("1", "1"), &metadata), &media)
        .expect("file")
        .write(false)
        .expect("keep");
    TvShowNfo::new(
        tv_show(&anime_id(100), &metadata).expect("titles"),
        dir.path(),
    )
    .expect("dir")
    .write(false, None)
    .expect("keep");

    assert_eq!(std::fs::read_to_string(&nfo_path).expect("read"), "old");
    assert_eq!(mtime(&nfo_path), old_time);
    assert_eq!(std::fs::read_to_string(&tv_path).expect("read"), "old");
    assert_eq!(mtime(&tv_path), old_time);
}

#[test]
fn overwrite_rewrites_the_nfo_and_sets_the_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    let metadata = make_show();
    let media = dir.path().join("Show - 01.mkv");
    std::fs::write(&media, "").expect("media");
    let nfo_path = media.with_extension("nfo");
    std::fs::write(&nfo_path, "old").expect("seed");

    EpisodeNfo::new(episodes(&file("1", "1"), &metadata), &media)
        .expect("file")
        .write(true)
        .expect("force");

    assert!(std::fs::read_to_string(&nfo_path)
        .expect("read")
        .contains("<episodedetails>"));
    assert_eq!(Some(mtime(&nfo_path)), modified_time("2020-01-08"));
}

#[test]
fn a_missing_date_leaves_the_time_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let metadata = make_show();
    let media = dir.path().join("Show - 03.mkv");
    std::fs::write(&media, "").expect("media");
    let nfo_path = media.with_extension("nfo");
    std::fs::write(&nfo_path, "old").expect("seed");
    let before = mtime(&nfo_path);

    EpisodeNfo::new(episodes(&file("3", "3"), &metadata), &media)
        .expect("file")
        .write(false)
        .expect("keep");
    assert_eq!(mtime(&nfo_path), before);

    EpisodeNfo::new(episodes(&file("3", "3"), &metadata), &media)
        .expect("file")
        .write(true)
        .expect("force");
    assert!(mtime(&nfo_path) >= before);
    assert!(mtime(&nfo_path) > modified_time("2021-01-01").expect("date"));
}
