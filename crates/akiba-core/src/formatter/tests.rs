use std::path::Path;

use super::*;
use crate::config::Config;

/// The sample record, which is also `anidb-file.fid.toml`.
fn sample() -> FileInfo {
    serde_json::from_value(fixture_rows()[0]["data"].clone())
        .expect("fixture row 0 data is a FileInfo")
}

fn sample_with(overrides: impl FnOnce(&mut FileInfo)) -> FileInfo {
    let mut info = sample();
    overrides(&mut info);
    info
}

fn fixture_rows() -> Vec<serde_json::Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formatter-cases.json");
    let text = std::fs::read_to_string(path).expect("read formatter fixture");
    serde_json::from_str(&text).expect("formatter fixture is JSON")
}

/// Formats `template` for `info`, the way the renamer does.
fn format(template: &str, info: &FileInfo) -> String {
    Template::parse(template)
        .and_then(|parsed| parsed.render(info))
        .expect("template formats")
}

fn name(value: &str) -> String {
    let info = sample_with(|info| info.anime_name_romaji = value.to_owned());
    format("{anime_name_romaji}", &info)
}

#[test]
fn drops_trailing_dots_before_an_extension() {
    let info = sample_with(|info| info.episode_name = "Cut My Losses...".to_owned());
    assert_eq!(format("{episode_name}.mkv", &info), "Cut My Losses.mkv");
    assert_eq!(
        format("{episode_name} - x.mkv", &info),
        "Cut My Losses... - x.mkv"
    );
    // only the extension, a dot in the middle of the template stays
    assert_eq!(
        format("{episode_name}.{crc32}.mkv", &info),
        "Cut My Losses....438a7b93.mkv"
    );
}

#[test]
fn tags_match_file_info_fields() {
    // serde_json maps are sorted, so compare as sorted lists
    let value = serde_json::to_value(FileInfo::default()).expect("serialises");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    let mut tags: Vec<&str> = FIELDS.iter().map(|(name, _)| *name).collect();
    keys.sort_unstable();
    tags.sort_unstable();
    assert_eq!(keys, tags);
    assert_eq!(FIELDS.len(), keys.len());
}

/// Fixture rows where the `number` modifier parses strictly: the whole value
/// must be an integer (or two joined by `-`), otherwise it is kept as written.
/// The fixture file holds the leading digits as output, each entry here is
/// `(episode, fixture output, expected output)`. No realistic `AniDB` value
/// (plain digits, prefixed values, ranges) is in this list.
const NUMBER_STRICT_CASES: [(&str, &str, &str); 4] = [
    ("1-2-3", "1", "1-2-3"),
    ("12abc", "12", "12abc"),
    ("1.5", "1", "1.5"),
    ("01-02v2", "1-2", "01-02v2"),
];

#[test]
fn fixture_matrix_matches() {
    let rows = fixture_rows();
    assert_eq!(rows.len(), 88);
    let mut overridden = 0;
    for row in &rows {
        let template = row["template"].as_str().expect("template");
        let info: FileInfo = serde_json::from_value(row["data"].clone())
            .unwrap_or_else(|err| panic!("data of {template:?}: {err}"));
        let result = Template::parse(template).and_then(|parsed| parsed.render(&info));
        if let Some(output) = row.get("output") {
            let mut expected = output.as_str().expect("output");
            if template.ends_with(":number}") || template.ends_with(":NUMBER}") {
                if let Some((_, fixture_output, strict)) = NUMBER_STRICT_CASES
                    .iter()
                    .find(|(episode, _, _)| *episode == info.episode)
                {
                    assert_eq!(
                        expected, *fixture_output,
                        "fixture output of {:?}",
                        info.episode
                    );
                    expected = strict;
                    overridden += 1;
                }
            }
            assert_eq!(result, Ok(expected.to_owned()), "template {template:?}");
        } else {
            let error = &row["error"];
            match error["name"].as_str().expect("error name") {
                "format" => assert_eq!(
                    result.map_err(|err| err.to_string()),
                    Err(error["message"].as_str().expect("message").to_owned()),
                    "template {template:?}"
                ),
                // only the kind of error is compared, not its message
                "number_element" => assert!(
                    matches!(result, Err(FormatError::NumberElement { .. })),
                    "template {template:?} should fail with a number element error, got {result:?}"
                ),
                other => panic!("unexpected error name {other}"),
            }
        }
    }
    assert_eq!(overridden, NUMBER_STRICT_CASES.len());
}

#[test]
fn applies_tags_modifiers_and_array_indexes() {
    let data = sample_with(|info| {
        info.anime_name_romaji = "Mixed Case".to_owned();
        info.episode = "013".to_owned();
        info.anime_category = vec!["Action".to_owned(), "Comedy".to_owned()];
    });

    assert_eq!(
        format(
            "{anime_name_romaji} - {episode} ({crc32}).{filetype}",
            &data
        ),
        "Mixed Case - 013 (438a7b93).mkv"
    );
    assert_eq!(format("{aid}", &data), "10914");

    assert_eq!(format("{anime_name_romaji:upper}", &data), "MIXED CASE");
    assert_eq!(format("{anime_name_romaji:LOWER}", &data), "mixed case");
    assert_eq!(format("{anime_name_romaji:upper_first}", &data), "M");
    assert_eq!(format("{anime_name_romaji:lower_first}", &data), "m");

    assert_eq!(format("{episode:number}", &data), "13");
    assert_eq!(format("{aid:number}", &data), "10914");
    for (episode, expected) in [("01-03", "1-3"), ("abc", "abc"), ("01-x", "01-x")] {
        let info = sample_with(|info| info.episode = episode.to_owned());
        assert_eq!(format("{episode:number}", &info), expected);
    }

    assert_eq!(format("{anime_category}", &data), "Action");
    assert_eq!(format("{anime_category:1}", &data), "Comedy");
    assert_eq!(format("{anime_category:5}", &data), "Action,Comedy");
}

#[test]
fn sanitizes_tag_values_but_not_the_template() {
    assert_eq!(name("AC/DC"), "ACDC");
    assert_eq!(name("It`s"), "It's");
    assert_eq!(name("a:b*c"), "a_b_c");
    assert_eq!(name("a:?*b"), "a_b");
    assert_eq!(name(":abc:"), "abc");
    assert_eq!(name("abc:."), "abc.");
    assert_eq!(name("abc: def"), "abc def");
    assert_eq!(name("A & B! (2020), ~+"), "A & B! (2020), ~+");

    assert_eq!(format("a:b/{crc32}", &sample()), "a:b/438a7b93");
}

#[test]
fn cleans_up_directory_parts_and_path_traversal() {
    let data = sample_with(|info| info.anime_name_romaji = "Test Anime".to_owned());

    assert_eq!(
        format("{anime_name_romaji}./Season 1./{crc32}.{filetype}", &data),
        "Test Anime/Season 1/438a7b93.mkv"
    );
    assert_eq!(
        format("{anime_name_romaji}/{anime_name_romaji}.", &data),
        "Test Anime/Test Anime."
    );

    let slash = sample_with(|info| info.anime_name_romaji = "/etc".to_owned());
    assert_eq!(format("x{anime_name_romaji:upper_first}", &slash), "x");
    assert_eq!(format("x{anime_name_romaji:lower_first}", &slash), "x");
    assert_eq!(name("../../etc"), "....etc");

    let dots = sample_with(|info| info.anime_name_romaji = "..".to_owned());
    assert_eq!(format("{anime_name_romaji}/file", &dots), "./file");
    assert_eq!(format("a/{anime_name_romaji}", &dots), "a/..");
}

#[test]
fn unknown_tags_fail_at_parse_with_the_known_tags_message() {
    let error = Template::parse("{nope}").expect_err("unknown tag");
    assert!(matches!(error, FormatError::UnknownTag { ref token } if token == "{nope}"));
    assert!(error.to_string().starts_with(
        "The tag {nope} is not known, available tags: fid, aid, eid, gid, lid, status, size, ed2k"
    ));
    assert!(
        Template::parse("{quality}").is_err(),
        "the field is spelled quaility"
    );
    assert!(Template::parse("{ANIME_NAME_ROMAJI}").is_err());
}

#[test]
fn validation_accepts_known_tags_and_rejects_unknown() {
    assert!(Template::parse("{anime_name_romaji} - {episode:number}.{filetype}").is_ok());
    assert!(Template::parse("{nope}").is_err());
}

#[test]
fn default_format_parses() {
    assert!(Template::parse(&Config::default().renamer.format).is_ok());
}

#[test]
fn non_tag_braces_are_literal() {
    let info = sample();
    assert_eq!(format("{episode-name}", &info), "{episode-name}");
    assert_eq!(format("{anime_category:-1}", &info), "{anime_category:-1}");
    assert_eq!(format("{{episode}}", &info), "{1}");
    assert_eq!(format("{ep isode}", &info), "{ep isode}");
}

#[test]
fn sanitize_tag_follows_the_replacement_chain() {
    assert_eq!(sanitize_tag("a__b"), "a_b");
    assert_eq!(sanitize_tag("_a_"), "a");
    assert_eq!(sanitize_tag("x_._y"), "x._y");
    assert_eq!(sanitize_tag("x_!"), "x!");
    assert_eq!(sanitize_tag("x_-.y"), "x-y");
    assert_eq!(sanitize_tag("x _y"), "x y");
    assert_eq!(sanitize_tag("x_ y"), "x y");
    assert_eq!(sanitize_tag("日本"), "");
    assert_eq!(sanitize_tag("\u{1f600}a"), "a");
}

#[test]
fn dir_components_drop_one_trailing_dot_except_the_basename() {
    assert_eq!(sanitize_dir_components("a./b./c"), "a/b/c");
    assert_eq!(sanitize_dir_components("dir./"), "dir./");
    assert_eq!(sanitize_dir_components("Season 1./Show."), "Season 1/Show.");
    // a value equal to the basename keeps its dot, even when it is not last
    assert_eq!(sanitize_dir_components("Show./Show."), "Show./Show.");
    assert_eq!(sanitize_dir_components("a/.."), "a/..");
    assert_eq!(sanitize_dir_components(""), "");
}

#[test]
fn number_parses_whole_integers_only() {
    let number_of = |text: &str| {
        let info = sample_with(|info| info.episode = text.to_owned());
        format("{episode:number}", &info)
    };
    assert_eq!(number_of("00"), "0");
    // two parts, and the empty side is not a number, so the value is sanitised as is
    assert_eq!(number_of("-0"), "-0");
    assert_eq!(number_of("-1"), "-1");
    assert_eq!(number_of(" 7"), "7");
    assert_eq!(number_of("+3"), "3");
    assert_eq!(number_of("01-03"), "1-3");
    assert_eq!(number_of(""), "");
    // junk after the digits is kept as written, nothing is cut off
    assert_eq!(number_of("12abc"), "12abc");
    assert_eq!(number_of("1.5"), "1.5");
    assert_eq!(number_of("1e3"), "1e3");
    assert_eq!(number_of("0x10"), "0x10");
    assert_eq!(number_of("01-02v2"), "01-02v2");
    assert_eq!(number_of("1-2-3"), "1-2-3");
}

#[test]
fn number_keeps_specials_as_written() {
    let number_of = |text: &str| {
        let info = sample_with(|info| info.episode = text.to_owned());
        format("{episode:number}", &info)
    };
    for prefix in ['S', 'C', 'T', 'P', 'O', 'E'] {
        for digits in ["1", "01", "12"] {
            let single = format!("{prefix}{digits}");
            assert_eq!(number_of(&single), single);
            let range = format!("{prefix}{digits}-{prefix}{digits}");
            assert_eq!(number_of(&range), range);
        }
    }
    // a range with one prefixed end is not a number range either
    assert_eq!(number_of("S01-03"), "S01-03");
}

#[test]
fn number_of_a_huge_value_is_kept_as_written() {
    let info = sample_with(|info| info.episode = "99999999999999999999999".to_owned());
    assert_eq!(format("{episode:number}", &info), "99999999999999999999999");
    let info = sample_with(|info| info.episode = "9223372036854775807".to_owned());
    assert_eq!(format("{episode:number}", &info), "9223372036854775807");
}

#[test]
fn array_index_must_be_all_digits() {
    let info = sample();
    assert_eq!(format("{anime_name_other:1}", &info), "TRINITY SEVEN  OAD");
    // not an index, so the array is joined
    assert_eq!(
        format("{anime_name_other:1abc}", &info),
        format("{anime_name_other:x}", &info)
    );
    assert_ne!(
        format("{anime_name_other:1abc}", &info),
        format("{anime_name_other:1}", &info)
    );
    // an index too large for usize is out of range, so the array is joined
    let data = sample_with(|info| {
        info.anime_category = vec!["Action".to_owned(), "Comedy".to_owned()];
    });
    assert_eq!(
        format("{anime_category:99999999999999999999999}", &data),
        "Action,Comedy"
    );
}

#[test]
fn first_char_modifiers_handle_non_bmp_and_empty() {
    let info = sample_with(|info| info.anime_name_romaji = "\u{1f600}abc".to_owned());
    assert_eq!(format("x{anime_name_romaji:upper_first}", &info), "x");
    let info = sample_with(|info| info.anime_name_romaji = String::new());
    assert_eq!(format("{anime_name_romaji:upper_first}", &info), "");
}

#[test]
fn number_element_is_an_error_only_for_the_default_branch() {
    let info = sample();
    assert!(matches!(
        Template::parse("{anime_year}").and_then(|t| t.render(&info)),
        Err(FormatError::NumberElement { .. })
    ));
    assert_eq!(format("{anime_year:number}", &info), "2015,2015");
    assert_eq!(format("{anime_year:5}", &info), "2015,2015");
    assert_eq!(format("{anime_year:upper}", &info), "2015,2015");
}
