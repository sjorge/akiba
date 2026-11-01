use super::*;
use crate::http::fake::{json as json_reply, Call, FakeHttp};
use serde_json::{json, Value};

const KEY: &str = "secret-key";

fn title(kind: &str, language: &str, text: &str) -> TitleVariant {
    TitleVariant {
        title: text.to_owned(),
        kind: Some(kind.to_owned()),
        year: None,
        language: language.to_owned(),
    }
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

/// Series with the Animation genre and the given names.
fn series(id: i64, name: &str, original: Option<&str>, language: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "original_name": original.unwrap_or(name),
        "original_language": language,
        "genre_ids": [16, 18],
    })
}

/// A stand-in for TMDB. `searches` maps the query to its results.
fn tmdb_fake(searches: Vec<(&'static str, Vec<Value>)>) -> FakeHttp {
    FakeHttp::new(move |call: &Call| {
        // The encoded query, as it appears in the URL.
        let encoded = call
            .url
            .split("&query=")
            .nth(1)
            .and_then(|rest| rest.split('&').next())
            .expect("query parameter");
        let results = searches
            .iter()
            .find(|(known, _)| percent_encode(known) == encoded)
            .map_or(Vec::new(), |(_, results)| results.clone());
        json_reply(&json!({ "results": results }))
    })
}

#[test]
fn exact_match_on_the_english_title() {
    let fake = tmdb_fake(vec![(
        "Shingeki no Kyojin",
        vec![
            series(1, "Other", None, "en"),
            series(2, "Shingeki no Kyojin", None, "ja"),
        ],
    )]);
    let mut lookup = Tmdb::new(&fake, Some(KEY)).expect("key");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[title("main", "x-jat", "Shingeki no Kyojin")],
            false,
        )
        .expect("apply");
    assert_eq!(id.tmdb, Some(2));
    assert_eq!(id.tmdb_season, Some(1));
}

#[test]
fn results_without_the_animation_genre_are_ignored() {
    let fake = tmdb_fake(vec![(
        "Dorama",
        vec![json!({
            "id": 9,
            "name": "Dorama",
            "original_name": "Dorama",
            "original_language": "en",
            "genre_ids": [18],
        })],
    )]);
    let mut lookup = Tmdb::new(&fake, Some(KEY)).expect("key");
    let mut id = anime(1);
    lookup
        .apply(&mut id, &[title("main", "x-jat", "Dorama")], false)
        .expect("apply");
    assert_eq!(id.tmdb, None);
}

#[test]
fn the_first_exact_match_in_a_reply_wins() {
    let fake = tmdb_fake(vec![(
        "Alpha",
        vec![
            series(1, "Alpha", None, "en"),
            series(2, "Alpha", None, "en"),
        ],
    )]);
    let mut lookup = Tmdb::new(&fake, Some(KEY)).expect("key");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[
                title("main", "x-jat", "Alpha"),
                title("main", "x-jat", "Beta"),
            ],
            false,
        )
        .expect("apply");
    assert_eq!(id.tmdb, Some(1));
    let searched: Vec<_> = fake.calls().iter().map(|c| c.url.clone()).collect();
    assert_eq!(searched.len(), 1, "an exact match stops the title loop");
}

#[test]
fn japanese_titles_match_after_normalisation() {
    let fake = tmdb_fake(vec![(
        "Yuru & Camp [OVA],",
        vec![series(
            7,
            "Laid-Back Camp",
            Some("yuru and camp 【ova】、"),
            "ja",
        )],
    )]);
    let mut lookup = Tmdb::new(&fake, Some(KEY)).expect("key");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[title("official", "ja", "Yuru & Camp [OVA],")],
            false,
        )
        .expect("apply");
    assert_eq!(id.tmdb, Some(7));
    assert_eq!(id.tmdb_season, Some(1));
}

#[test]
fn normalisation_is_only_for_japanese_titles() {
    let fake = tmdb_fake(vec![(
        "Yuru & Camp [OVA],",
        vec![series(7, "Other", Some("yuru and camp 【ova】、"), "ja")],
    )]);
    let mut lookup = Tmdb::new(&fake, Some(KEY)).expect("key");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[title("official", "en", "Yuru & Camp [OVA],")],
            false,
        )
        .expect("apply");
    assert_eq!(id.tmdb, None);
}

#[test]
fn normalisation_rules_match_the_expected_values() {
    assert_eq!(normalise_japanese("A & B"), "a and b");
    assert_eq!(normalise_japanese("[X]"), "【x】");
    assert_eq!(normalise_japanese("End."), "end。");
    assert_eq!(normalise_japanese("a.b."), "a.b。");
    assert_eq!(normalise_japanese("1, 2"), "1、 2");
}

#[test]
fn the_search_url_is_the_moviedb_query_string() {
    let fake = tmdb_fake(vec![]);
    let mut lookup = Tmdb::new(&fake, Some(KEY)).expect("key");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[title("main", "x-jat", "Yuru & Camp [OVA],")],
            false,
        )
        .expect("apply");
    assert_eq!(
            fake.calls()[0].url,
            "https://api.themoviedb.org/3/search/tv?api_key=secret-key&query=Yuru%20%26%20Camp%20%5BOVA%5D%2C&include_adult=true"
        );
    assert_eq!(fake.calls()[0].method, "GET");
}

#[test]
fn a_mapped_id_is_not_looked_up_again() {
    let fake = FakeHttp::offline();
    let mut lookup = Tmdb::new(&fake, Some(KEY)).expect("key");
    let mut id = AnimeId {
        tmdb: Some(5),
        ..anime(1)
    };
    lookup
        .apply(&mut id, &[title("main", "x-jat", "Alpha")], false)
        .expect("apply");
    assert_eq!(id.tmdb, Some(5));
    assert_eq!(fake.calls(), []);
}

#[test]
fn errors_do_not_leak_the_api_key() {
    let fake = FakeHttp::new(|call| {
        Err(HttpError::new(
            &call.url,
            format!("failed for {}", call.url),
        ))
    });
    let mut lookup = Tmdb::new(&fake, Some(KEY)).expect("key");
    let mut id = anime(1);
    let err = lookup
        .apply(&mut id, &[title("main", "x-jat", "Alpha")], false)
        .expect_err("HTTP failure");
    let IdMapError::Http(err) = err else {
        panic!("expected an HTTP error, got {err:?}");
    };
    assert!(!err.url.contains(KEY), "{err}");
    assert!(!err.message.contains(KEY), "{err}");
    assert!(err.url.contains("api_key=***"), "{err}");
}

#[test]
fn a_missing_key_fails_construction() {
    let fake = FakeHttp::offline();
    let Err(err) = Tmdb::new(&fake, None) else {
        panic!("no key must fail");
    };
    assert!(matches!(err, IdMapError::MissingTmdbKey), "{err:?}");
    assert!(Tmdb::new(&fake, Some("")).is_err(), "empty key");
}
