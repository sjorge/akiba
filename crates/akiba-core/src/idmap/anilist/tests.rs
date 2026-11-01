use super::*;
use crate::http::fake::{json as json_reply, Call, FakeHttp};
use crate::http::HttpError;

const TOKEN: &str = "test-token";

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

fn media(id: i64, romaji: &str, native: Option<&str>) -> Value {
    json!({ "id": id, "title": { "romaji": romaji, "native": native } })
}

/// A stand-in for `AniList`. `searches` maps a search term to its media, and
/// `seasons` maps an id to its season year. Unknown terms find nothing.
fn anilist_fake(searches: Vec<(&'static str, Value)>, seasons: Vec<(i64, i64)>) -> FakeHttp {
    FakeHttp::new(move |call: &Call| {
        assert_eq!(call.url, ENDPOINT);
        let body = call.body.as_ref().expect("POST body");
        let variables = &body["variables"];
        let reply = if body["query"].as_str() == Some(SEARCH_QUERY) {
            let term = variables["search"].as_str().expect("search term");
            let media = searches
                .iter()
                .find(|(known, _)| *known == term)
                .map_or(json!([]), |(_, media)| media.clone());
            json!({ "data": { "Page": { "media": media } } })
        } else {
            assert_eq!(body["query"].as_str(), Some(MEDIA_QUERY));
            let id = variables["id"].as_i64().expect("media id");
            let year = seasons
                .iter()
                .find(|(known, _)| *known == id)
                .map_or(Value::Null, |(_, year)| json!(year));
            json!({ "data": { "Media": { "id": id, "seasonYear": year } } })
        };
        json_reply(&reply)
    })
}

fn searches(fake: &FakeHttp) -> Vec<String> {
    fake.calls()
        .iter()
        .filter_map(|call| {
            call.body.as_ref()?["variables"]["search"]
                .as_str()
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn exact_match_on_the_romaji_title() {
    let fake = anilist_fake(
        vec![(
            "Shingeki no Kyojin",
            json!([
                media(99, "Other", None),
                media(16498, "Shingeki no Kyojin", None)
            ]),
        )],
        vec![],
    );
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[title("main", "x-jat", "Shingeki no Kyojin")],
            false,
        )
        .expect("apply");
    assert_eq!(id.anilist, Some(16498));
    assert_eq!(fake.calls().len(), 1, "an exact match needs no season call");
}

#[test]
fn an_exact_match_is_case_insensitive() {
    let fake = anilist_fake(
        vec![(
            "SHINGEKI NO KYOJIN",
            json!([media(16498, "Shingeki no Kyojin", None)]),
        )],
        vec![],
    );
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[title("main", "x-jat", "SHINGEKI NO KYOJIN")],
            false,
        )
        .expect("apply");
    assert_eq!(id.anilist, Some(16498));
}

#[test]
fn fuzzy_match_takes_the_closest_title() {
    let fake = anilist_fake(
        vec![(
            "Shingeki no Kyojinn",
            json!([
                media(20, "Shingeki no Kyojin Season 2", None),
                media(16498, "Shingeki no Kyojin", None),
            ]),
        )],
        vec![],
    );
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[title("main", "x-jat", "Shingeki no Kyojinn")],
            false,
        )
        .expect("apply");
    assert_eq!(id.anilist, Some(16498));
}

#[test]
fn a_distant_title_is_not_a_match() {
    let fake = anilist_fake(
        vec![(
            "Completely Different Title",
            json!([media(16498, "Shingeki no Kyojin", None)]),
        )],
        vec![],
    );
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[title("main", "x-jat", "Completely Different Title")],
            false,
        )
        .expect("apply");
    assert_eq!(id.anilist, None);
}

#[test]
fn japanese_titles_match_the_native_title_with_a_tighter_threshold() {
    // Five edits is within the Japanese threshold of 5, six is not.
    let fake = anilist_fake(
        vec![
            (
                "進撃の巨人",
                json!([media(16498, "Shingeki no Kyojin", Some("進撃の巨人"))]),
            ),
            (
                "進撃の巨人abcde",
                json!([media(16498, "Shingeki no Kyojin", Some("進撃の巨人"))]),
            ),
            (
                "進撃の巨人abcdef",
                json!([media(16498, "Shingeki no Kyojin", Some("進撃の巨人"))]),
            ),
        ],
        vec![],
    );
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");

    let mut exact = anime(1);
    lookup
        .apply(&mut exact, &[title("official", "ja", "進撃の巨人")], false)
        .expect("apply");
    assert_eq!(exact.anilist, Some(16498));

    let mut five = anime(2);
    lookup
        .apply(
            &mut five,
            &[title("official", "ja", "進撃の巨人abcde")],
            false,
        )
        .expect("apply");
    assert_eq!(five.anilist, Some(16498));

    let mut six = anime(3);
    lookup
        .apply(
            &mut six,
            &[title("official", "ja", "進撃の巨人abcdef")],
            false,
        )
        .expect("apply");
    assert_eq!(six.anilist, None);
}

#[test]
fn a_year_variant_matches_the_season_year() {
    let fake = anilist_fake(
        vec![
            ("Show (2013)", json!([])),
            (
                "Show",
                json!([media(2, "Show", None), media(1, "Show", None)]),
            ),
        ],
        vec![(1, 2013), (2, 2020)],
    );
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    lookup
        .apply(&mut id, &[title("main", "x-jat", "Show (2013)")], false)
        .expect("apply");
    assert_eq!(id.anilist, Some(1));
    assert_eq!(searches(&fake), ["Show (2013)", "Show"]);
}

#[test]
fn plain_titles_are_searched_before_year_variants() {
    let fake = anilist_fake(vec![("Show", json!([]))], vec![]);
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    lookup
        .apply(
            &mut id,
            &[
                title("main", "x-jat", "Show (2013)"),
                title("main", "x-jat", "Other"),
            ],
            false,
        )
        .expect("apply");
    assert_eq!(searches(&fake), ["Show (2013)", "Other", "Show"]);
}

#[test]
fn an_exact_match_stops_the_remaining_searches() {
    let fake = anilist_fake(vec![("Alpha", json!([media(1, "Alpha", None)]))], vec![]);
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
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
    assert_eq!(id.anilist, Some(1));
    assert_eq!(searches(&fake), ["Alpha"]);
}

#[test]
fn a_mapped_id_is_not_looked_up_again() {
    let fake = FakeHttp::offline();
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = AnimeId {
        anilist: Some(5),
        ..anime(1)
    };
    lookup
        .apply(&mut id, &[title("main", "x-jat", "Alpha")], false)
        .expect("apply");
    assert_eq!(id.anilist, Some(5));
    assert_eq!(fake.calls(), []);
}

#[test]
fn overwrite_keeps_the_mapped_id_when_nothing_is_found() {
    let fake = anilist_fake(vec![], vec![]);
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = AnimeId {
        anilist: Some(5),
        ..anime(1)
    };
    lookup
        .apply(&mut id, &[title("main", "x-jat", "Alpha")], true)
        .expect("apply");
    assert_eq!(id.anilist, Some(5));
}

#[test]
fn requests_carry_the_bearer_token_and_the_search_variables() {
    let fake = anilist_fake(vec![], vec![]);
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    lookup
        .apply(&mut id, &[title("main", "x-jat", "Alpha")], false)
        .expect("apply");

    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].header("authorization"), Some("Bearer test-token"));
    assert_eq!(
        calls[0].body.as_ref().expect("body")["variables"],
        json!({ "search": "Alpha", "page": 1, "perPage": 5 })
    );
}

#[test]
fn graphql_errors_are_no_match() {
    let fake = FakeHttp::new(|_| {
        json_reply(&json!({ "errors": [{ "message": "Too Many Requests." }], "data": null }))
    });
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    lookup
        .apply(&mut id, &[title("main", "x-jat", "Alpha")], false)
        .expect("apply");
    assert_eq!(id.anilist, None);
}

#[test]
fn http_failures_are_errors() {
    let fake = FakeHttp::new(|call| Err(HttpError::new(&call.url, "HTTP status 500")));
    let mut lookup = AniList::new(&fake, Some(TOKEN)).expect("token");
    let mut id = anime(1);
    let err = lookup
        .apply(&mut id, &[title("main", "x-jat", "Alpha")], false)
        .expect_err("HTTP failure");
    assert!(matches!(err, IdMapError::Http(_)), "{err:?}");
}

#[test]
fn a_missing_token_fails_construction() {
    let fake = FakeHttp::offline();
    let Err(err) = AniList::new(&fake, None) else {
        panic!("no token must fail");
    };
    assert!(matches!(err, IdMapError::MissingAnilistToken), "{err:?}");
    assert!(AniList::new(&fake, Some("")).is_err(), "empty token");
}

#[test]
fn year_suffix_is_split_off_only_when_it_is_a_year() {
    let split = year_variant(&title("main", "x-jat", "Show (2013)")).expect("split");
    assert_eq!(split.title, "Show");
    assert_eq!(split.year, Some(2013));
    assert!(year_variant(&title("main", "x-jat", "Show (13)")).is_none());
    assert!(year_variant(&title("main", "x-jat", "Show 2013")).is_none());
}

#[test]
fn split_year_needs_a_bracketed_four_digit_year_at_the_end() {
    assert_eq!(split_year("Show (2013)"), Some(("Show", 2013)));
    assert_eq!(
        split_year("Show: Part 2 (2021)"),
        Some(("Show: Part 2", 2021))
    );
    assert_eq!(
        split_year("Show (2013) (2014)"),
        Some(("Show (2013)", 2014))
    );
    assert_eq!(split_year("Show  (0001)"), Some(("Show ", 1)));
    assert_eq!(split_year("Show\u{3000}(2013)"), Some(("Show", 2013)));
    assert_eq!(split_year("Show (20133)"), None);
    assert_eq!(split_year("Show (201)"), None);
    assert_eq!(split_year("Show (20a3)"), None);
    assert_eq!(split_year("Show(2013)"), None);
    assert_eq!(split_year("Show (2013) OVA"), None);
    assert_eq!(split_year("Show (2013"), None);
    assert_eq!(split_year(" (2013)"), None);
    assert_eq!(split_year("(2013)"), None);
    assert_eq!(split_year("Show 2013"), None);
    assert_eq!(split_year("Show (٢٠١٣)"), None);
}

/// The separator is Unicode `White_Space`, as in `char::is_whitespace`: U+0085
/// (NEL) counts, U+FEFF (BOM) does not, and a line break in the name is kept.
#[test]
fn split_year_whitespace_follows_the_rust_definition() {
    assert_eq!(split_year("Show\u{85}(2013)"), Some(("Show", 2013)));
    assert_eq!(split_year("Show\u{FEFF}(2013)"), None);
    assert_eq!(
        split_year("Line\nbreak (2013)"),
        Some(("Line\nbreak", 2013))
    );
}
