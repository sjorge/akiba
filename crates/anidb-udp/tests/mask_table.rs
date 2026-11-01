//! Checks the mask tables in `src/mask.rs` against reference values.
//!
//! `fixtures/masks.toml` holds reference values computed independently, one field
//! at a time, so every row's byte and bit are verified on their own. The renamer
//! field list is covered separately in the crate's unit tests.

use std::collections::BTreeMap;

use anidb_udp::mask::{AnimeField, AnimeFields, Field, FileField, FileFields};

const ORACLE: &str = include_str!("fixtures/masks.toml");

fn name_of<F: Field>(field: F) -> &'static str {
    F::TABLE
        .iter()
        .find(|spec| spec.field == field)
        .expect("every field has a row")
        .name
}

#[test]
fn every_row_matches_its_reference_mask() {
    let oracle: toml::Table = toml::from_str(ORACLE).unwrap();
    let single = oracle["single"].as_table().unwrap();
    assert_eq!(
        single.len(),
        FileField::TABLE.len() + AnimeField::TABLE.len()
    );

    for spec in FileField::TABLE {
        let mask = single[spec.name].as_str().unwrap();
        let set: FileFields = [spec.field].into_iter().collect();
        assert_eq!(set.to_hex(), &mask[..10], "fmask of {}", spec.name);
    }
    for spec in AnimeField::TABLE {
        let mask = single[spec.name].as_str().unwrap();
        let set: AnimeFields = [spec.field].into_iter().collect();
        assert_eq!(set.to_hex(), &mask[10..], "amask of {}", spec.name);
    }
}

#[test]
fn full_masks_and_reply_order_match_the_reference() {
    let oracle: toml::Table = toml::from_str(ORACLE).unwrap();
    let file: FileFields = FileField::TABLE.iter().map(|spec| spec.field).collect();
    let anime: AnimeFields = AnimeField::TABLE.iter().map(|spec| spec.field).collect();

    assert_eq!(file.to_hex(), oracle["full_fmask"].as_str().unwrap());
    assert_eq!(anime.to_hex(), oracle["full_amask"].as_str().unwrap());

    let order: Vec<&str> = file
        .iter()
        .map(name_of)
        .chain(anime.iter().map(name_of))
        .collect();
    let expected: Vec<&str> = oracle["order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(order, expected);
}

#[test]
fn table_names_are_the_reference_names() {
    let oracle: toml::Table = toml::from_str(ORACLE).unwrap();
    let single: BTreeMap<&str, &str> = oracle["single"]
        .as_table()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str().unwrap()))
        .collect();
    let rust_names: Vec<&str> = FileField::TABLE
        .iter()
        .map(|s| s.name)
        .chain(AnimeField::TABLE.iter().map(|s| s.name))
        .collect();
    let mut sorted = rust_names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), rust_names.len());
    for name in rust_names {
        assert!(single.contains_key(name), "{name} is not in the reference");
    }
}
