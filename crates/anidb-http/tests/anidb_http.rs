use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use anidb_http::{
    fetch_poster, parse_anime, parse_titles, poster_url, AniDbShow, Error, HttpApi, TitleVariant,
    Transport, TransportError,
};

fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(p).unwrap()
}

fn aid_record(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn xml_maps_to_fixture_cache_values() {
    let show = parse_anime(&fixture("anime-100.xml")).unwrap();
    let ours: toml::Table = toml::from_str(&toml::to_string(&show).unwrap()).unwrap();
    let legacy: toml::Table = toml::from_str(&aid_record("anime-100.aid.toml")).unwrap();
    assert_eq!(ours, legacy);
}

#[test]
fn legacy_cache_deserialises_and_roundtrips() {
    let show: AniDbShow = toml::from_str(&aid_record("anime-100.aid.toml")).unwrap();
    assert_eq!(show.id, 100);
    assert_eq!(show.episodes[3].episode_number, "S1");
    assert_eq!(show.episodes[3].kind, 2);
    assert_eq!(show, parse_anime(&fixture("anime-100.xml")).unwrap());
}

#[test]
fn json_uses_camel_case_names() {
    let show = parse_anime(&fixture("anime-100.xml")).unwrap();
    let t = toml::to_string(&show).unwrap();
    for key in [
        "ageRestricted",
        "episodeCount",
        "startDate",
        "updatedAt",
        "episodeNumber",
        "airDate",
    ] {
        assert!(t.contains(key), "missing {key}");
    }
}

#[test]
fn extra_fields_are_mapped() {
    let xml = br#"<anime id="7" restricted="true">
  <type>Movie</type><episodecount>1</episodecount>
  <ratings><permanent count="10">8.5</permanent><temporary count="3">7.0</temporary></ratings>
  <relatedanime><anime id="8" type="Sequel">Next</anime></relatedanime>
  <tags><tag id="1" parentid="2" weight="300" localspoiler="false" globalspoiler="true" update="2020-01-01">
    <name>action</name><description>Fights</description><picurl>t.jpg</picurl></tag></tags>
  <characters><character id="5" type="main" update="2020-01-01"><rating votes="12">9.1</rating>
    <name>Hero</name><gender>male</gender><charactertype id="1">Character</charactertype>
    <description>d</description><picture>c.jpg</picture>
    <seiyuu id="9" picture="s.jpg">Voice</seiyuu></character></characters>
  <episodes><episode id="1" update="x"><epno type="1">1</epno><length>5</length><airdate>2020</airdate>
    <rating votes="4">3.5</rating><title xml:lang="en">Ep</title><summary>Sum &amp; more</summary></episode></episodes>
</anime>"#;
    let s = parse_anime(xml).unwrap();
    assert!(s.age_restricted);
    assert_eq!(s.ratings.unwrap().permanent.score.as_deref(), Some("8.5"));
    assert_eq!(s.related[0].id, 8);
    assert_eq!(s.tags[0].parent_id, Some(2));
    assert!(s.tags[0].global_spoiler);
    assert_eq!(s.tags[0].picture_url.as_deref(), Some("t.jpg"));
    assert_eq!(s.characters[0].rating, Some(9.1));
    assert_eq!(s.characters[0].seiyuu[0].picture.as_deref(), Some("s.jpg"));
    assert_eq!(
        s.characters[0].character_type.as_ref().unwrap().name,
        "Character"
    );
    assert_eq!(s.episodes[0].votes, Some(4));
    assert_eq!(s.episodes[0].summary.as_deref(), Some("Sum & more"));
}

#[test]
fn titles_dump_matches_expected_map() {
    let tv = |t: &str, k: &str, l: &str| TitleVariant {
        title: t.into(),
        kind: Some(k.into()),
        language: l.into(),
    };
    let expected: BTreeMap<u32, Vec<TitleVariant>> = BTreeMap::from([
        (
            1,
            vec![
                tv("Anime One", "main", "x-jat"),
                tv("Anime One", "official", "en"),
            ],
        ),
        (
            2,
            vec![
                tv("Fish & Chips \"Live\"", "main", "x-jat"),
                tv("Tom & Jerry", "official", "en"),
            ],
        ),
        (3, vec![]),
        (100, vec![tv("テスト", "official", "ja")]),
    ]);
    let xml = fixture("anime-titles.xml");
    assert_eq!(parse_titles(&xml).unwrap(), expected);

    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&xml).unwrap();
    assert_eq!(parse_titles(&gz.finish().unwrap()).unwrap(), expected);
}

type Seen = Vec<(String, Vec<(String, String)>)>;
type Check = fn(&Error) -> bool;

struct Fake {
    body: Vec<u8>,
    seen: RefCell<Seen>,
}

impl Fake {
    fn new(body: &[u8]) -> Self {
        Self {
            body: body.to_vec(),
            seen: RefCell::default(),
        }
    }
}

impl Transport for Fake {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, TransportError> {
        self.seen.borrow_mut().push((
            url.into(),
            headers
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        ));
        Ok(self.body.clone())
    }
}

fn api(f: &Fake) -> HttpApi<&Fake> {
    HttpApi::with_transport("http://x/httpapi", "my client", "2", "test-agent/1.0", f)
}

#[test]
fn api_builds_request_and_parses() {
    let f = Fake::new(&fixture("anime-100.xml"));
    let show = api(&f).anime(100).unwrap();
    assert_eq!(show.id, 100);
    let seen = f.seen.borrow();
    assert_eq!(
        seen[0].0,
        "http://x/httpapi?client=my%20client&clientver=2&protover=1&request=anime&aid=100"
    );
    assert!(seen[0]
        .1
        .contains(&("User-Agent".into(), "test-agent/1.0".into())));
}

#[test]
fn api_error_codes() {
    let cases: [(&str, Check); 5] = [
        (
            r#"<error code="302">client version missing or invalid</error>"#,
            |e| matches!(e, Error::ClientInvalid),
        ),
        (r#"<error code="500">banned</error>"#, |e| {
            matches!(e, Error::Banned)
        }),
        ("<error>aid Missing or Invalid</error>", |e| {
            matches!(e, Error::AnimeNotFound)
        }),
        ("<error>Anime not found</error>", |e| {
            matches!(e, Error::AnimeNotFound)
        }),
        (
            r#"<error code="310">illegal input or access denied</error>"#,
            |e| matches!(e, Error::Other(310)),
        ),
    ];
    for (body, check) in cases {
        let f = Fake::new(body.as_bytes());
        let err = api(&f).anime(1).unwrap_err();
        assert!(check(&err), "{body} -> {err:?}");
    }
}

#[test]
fn poster_url_points_at_the_anidb_cdn() {
    assert_eq!(
        poster_url("12345.jpg"),
        "https://cdn.anidb.net/images/main/12345.jpg"
    );
}

#[test]
fn fetch_poster_gets_the_cdn_url_and_returns_the_bytes() {
    let f = Fake::new(b"\xff\xd8\xff JPEG");
    let bytes = fetch_poster(&&f, "12345.jpg").unwrap();
    assert_eq!(bytes, b"\xff\xd8\xff JPEG");
    let seen = f.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, "https://cdn.anidb.net/images/main/12345.jpg");
    assert!(seen[0].1.is_empty(), "no headers for a plain image GET");
}

struct Refused;

impl Transport for Refused {
    fn get(&self, _url: &str, _headers: &[(&str, &str)]) -> Result<Vec<u8>, TransportError> {
        Err(TransportError::Status(404))
    }
}

#[test]
fn fetch_poster_reports_transport_errors() {
    let err = fetch_poster(&Refused, "missing.jpg").unwrap_err();
    assert!(
        matches!(err, Error::Transport(TransportError::Status(404))),
        "{err:?}"
    );
}

#[test]
fn a_lent_transport_serves_the_api_and_the_poster() {
    let f = Fake::new(b"poster");
    let transport: &dyn Transport = &f;
    assert_eq!(
        anidb_http::fetch_poster(&transport, "x.jpg").unwrap(),
        b"poster"
    );
    assert!(api(&f).anime(100).is_err()); // the body is not XML
    assert_eq!(f.seen.borrow().len(), 2);
}

#[test]
fn a_corrupt_gzip_is_a_decompression_error_with_its_source() {
    let err = parse_titles(&[0x1f, 0x8b, 0, 0]).unwrap_err();
    assert!(matches!(err, Error::Decompress(_)), "{err}");
    assert!(err.to_string().starts_with("decompression failed: "));
    assert!(std::error::Error::source(&err).is_some());
}
