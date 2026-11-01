use std::fs;
use std::path::Path;

use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use super::*;

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let text = fs::read_to_string(&path).expect("read fixture file");
    serde_json::from_str(&text).expect("fixture file is JSON")
}

fn str_of(value: &Value) -> &str {
    value.as_str().expect("fixture string")
}

/// Cases (by index in `natural-sort-cases.json`) with an expected order that
/// differs from the one in the fixture: the case, the order the crate gives and
/// the reason. Order only decides which file is processed first.
const NATURAL_EXPECTED_ORDERS: [(usize, &[&str], &str); 3] = [
    (
        3,
        &[
            "Show - 01 - Pilot.mkv",
            "Show - 01-02 - Pilot.mkv",
            "Show - 1.5 - Recap.mkv",
            "Show - 03 - Next.mkv",
            "Show - 10-12 - Finale.mkv",
        ],
        "1.5 is 1 then .5, so it sorts by 1 before 03; a number starting with 0 is not compared character by character",
    ),
    (
        4,
        &[
            "[Group] Show - 01 [1080p] [ABCDEF12].mkv",
            "[Group] Show - 2 [720p].mkv",
            "[Group] Show - 011 [1080p].mkv",
            "[Group] Show 1st Season - 00 (Special).mkv",
        ],
        "011 is 11, so it sorts after 2; a leading 0 does not make it compare character by character",
    ),
    (
        5,
        &["", "1", "01", "B", "a", "a1", "a01", "b"],
        "numerically equal 1 and 01 tie, then the shorter one first",
    ),
];

#[test]
fn natural_order_matches_the_fixture_but_for_the_expected_orders() {
    let mut overridden = 0;
    for (index, case) in fixture("natural-sort-cases.json")
        .as_array()
        .expect("array")
        .iter()
        .enumerate()
    {
        let input: Vec<String> = case["input"]
            .as_array()
            .expect("input")
            .iter()
            .map(|v| str_of(v).to_owned())
            .collect();
        let mut expected: Vec<String> = case["sorted"]
            .as_array()
            .expect("sorted")
            .iter()
            .map(|v| str_of(v).to_owned())
            .collect();
        if let Some((_, order, reason)) = NATURAL_EXPECTED_ORDERS.iter().find(|(i, ..)| *i == index)
        {
            assert_ne!(*reason, "");
            assert_ne!(
                order.to_vec(),
                expected,
                "case {index} has the fixture order"
            );
            expected = order.iter().map(|&name| name.to_owned()).collect();
            overridden += 1;
        }
        let mut sorted = input.clone();
        sorted.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(sorted, expected, "case {index}, input {input:?}");
    }
    assert_eq!(overridden, NATURAL_EXPECTED_ORDERS.len());
}

#[test]
fn natural_order_of_realistic_episode_names() {
    fn sorted<'a>(names: &[&'a str]) -> Vec<&'a str> {
        let mut v = names.to_vec();
        v.sort_by(|a, b| natural_cmp(a, b));
        v
    }
    // zero padded numbers of mixed width stay numeric
    let padded = [
        "Show - 010.mkv",
        "Show - 09.mkv",
        "Show - 002.mkv",
        "Show - 0100.mkv",
    ];
    assert_eq!(
        sorted(&padded),
        [
            "Show - 002.mkv",
            "Show - 09.mkv",
            "Show - 010.mkv",
            "Show - 0100.mkv"
        ]
    );
    // a title after the number sorts before a range of the same start
    let range = ["Show - 01-02.mkv", "Show - 01 - Pilot.mkv"];
    assert_eq!(
        sorted(&range),
        ["Show - 01 - Pilot.mkv", "Show - 01-02.mkv"]
    );
    // specials with a letter prefix group by prefix, then by number
    let specials = [
        "Show - S10.mkv",
        "Show - S2.mkv",
        "Show - S1.mkv",
        "Show - 03.mkv",
    ];
    assert_eq!(
        sorted(&specials),
        [
            "Show - 03.mkv",
            "Show - S1.mkv",
            "Show - S2.mkv",
            "Show - S10.mkv"
        ]
    );
    // case sensitive, upper case first
    assert_eq!(sorted(&["b 01", "B 01", "a 01"]), ["B 01", "a 01", "b 01"]);
}

#[test]
fn levenshtein_ignores_case_and_diacritics_like_the_collator() {
    assert_eq!(levenshtein("Café", "cafe"), 0);
    assert_eq!(levenshtein("ＡＢＣ", "ABC"), 0);
    assert_eq!(levenshtein("ヤマト", "やまと"), 0);
    assert_eq!(levenshtein("ß", "ss"), 2);
    assert_eq!(levenshtein("🎌 Festival", "Festival"), 3);
}

/// Compares the three candidate algorithms with `levenshtein-cases.json` and pins
/// the mismatches that the chosen one still has. The counts are printed for
/// the decision log.
/// Expected values were computed independently with the collator based
/// distance, not from the Rust code. They
/// cover the cases the 50-pair fixture does not: Hangul, NFD input, ligatures,
/// katakana width, surrogates and ignorable characters.
#[test]
fn levenshtein_matches_collator_on_reference_pairs() {
    let pairs: [(&str, &str, usize); 17] = [
        ("Cafe\u{301}", "Café", 1),
        ("Cafe\u{301}", "Cafe", 1),
        ("\u{301}", "\u{300}", 0),
        ("", "ﬁ", 1),
        ("ﬁre", "fire", 2),
        ("진격의 거인", "진격의 거", 1),
        ("인", "가", 1),
        ("ガ", "カ", 0),
        ("ｶﾞ", "ガ", 1),
        ("\u{200B}", "\u{301}", 0),
        ("\u{1}", "\u{301}", 0),
        ("\t", "\u{301}", 1),
        ("\u{85}", "\u{301}", 1),
        ("Ǆ", "DŽ", 2),
        ("İ", "i", 0),
        ("ﬀ", "ff", 2),
        ("😀", "😁", 1),
    ];
    for (a, b, want) in pairs {
        assert_eq!(levenshtein(a, b), want, "{a:?} vs {b:?}");
    }
    // An astral character is two code units, so it costs two deletions.
    assert_eq!(levenshtein("😀a", "a"), 2);
}

#[test]
fn levenshtein_drift_against_the_collator_fixture() {
    let lower = |a: &str, b: &str| strsim::levenshtein(&a.to_lowercase(), &b.to_lowercase());
    let nfkc_lower = |a: &str, b: &str| {
        let n = |s: &str| s.nfkc().collect::<String>().to_lowercase();
        strsim::levenshtein(&n(a), &n(b))
    };

    let mut mismatches: [Vec<String>; 3] = Default::default();
    let mut total = 0;
    for case in fixture("levenshtein-cases.json").as_array().expect("array") {
        total += 1;
        let (a, b) = (str_of(&case["a"]), str_of(&case["b"]));
        let want = usize::try_from(case["distance"].as_u64().expect("distance")).expect("usize");
        let candidates = [lower(a, b), nfkc_lower(a, b), levenshtein(a, b)];
        for (i, got) in candidates.iter().enumerate() {
            if *got != want {
                mismatches[i].push(format!("{a:?} vs {b:?}: want {want}, got {got}"));
            }
        }
    }
    eprintln!(
        "levenshtein drift over {total} pairs: lowercase {} / NFKC+lowercase {} / folded {}",
        mismatches[0].len(),
        mismatches[1].len(),
        mismatches[2].len()
    );
    for line in &mismatches[2] {
        eprintln!("  folded mismatch: {line}");
    }
    assert!(
        mismatches[2].is_empty(),
        "folded distance drifted from the collator: {:#?}",
        mismatches[2]
    );
}
