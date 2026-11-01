use std::collections::BTreeMap;

use super::*;
use crate::fields::DateFlag;
use crate::mask::{Field, Value};

fn int(value: i64) -> Value {
    Value::Int(value)
}

fn text(value: &str) -> Value {
    Value::Str(value.to_owned())
}

fn strs(items: &[&str]) -> Value {
    Value::Strs(items.iter().map(|s| (*s).to_owned()).collect())
}

fn ints(items: &[i64]) -> Value {
    Value::Ints(items.to_vec())
}

fn line(text: &str) -> Vec<String> {
    text.split('|').map(str::to_owned).collect()
}

/// The full mask line of an ANIME reply with every field requested, from a
/// reference value computed independently.
const FULL_LINE: &str = "10914|5|1998-2000|TV Series|123'456|1'2|Romaji Name|漢字|English Name|other a'other b|short1'short2|syn'|26|26|2|900000000|900500000|http://example.org/x|pic.jpg|850|120|845|30|870|4|award a'award b|1|1234|5678|nfo1|tag a,tag b|1,2|100,200|1600000000|1,2,3|1|2|3|4|5";

#[test]
fn masks_match_the_reference_hex() {
    // `AnimeParser` with every field, the default fields, and no fields
    assert_eq!(
        mask(AnimeCommandField::TABLE.iter().map(|spec| spec.field)).to_hex(),
        "fcfcfeff7f80f8"
    );
    assert_eq!(default_mask().to_hex(), "b0f0e0fc000000");
    assert_eq!(mask([]).to_hex(), "80000000000000");
}

#[test]
fn table_has_every_row_of_the_definition() {
    assert_eq!(AnimeCommandField::TABLE.len(), 40);
}

#[test]
fn decodes_every_field_of_the_full_reply() {
    let all = mask(AnimeCommandField::TABLE.iter().map(|spec| spec.field));
    let reply = AnimeReply::decode(&line(FULL_LINE), &all).unwrap();
    assert_eq!(reply.aid, 10_914);

    let expected: BTreeMap<AnimeCommandField, Value> = [
        (AnimeCommandField::Aid, int(10_914)),
        (
            AnimeCommandField::DateFlags,
            Value::DateFlags(vec![DateFlag::StartUnknownDay, DateFlag::EndUnknownDay]),
        ),
        (AnimeCommandField::Year, text("1998-2000")),
        (AnimeCommandField::Type, text("TV Series")),
        (AnimeCommandField::RelatedAnime, ints(&[123, 456])),
        (AnimeCommandField::RelatedTypes, ints(&[1, 2])),
        (AnimeCommandField::RomanjiName, text("Romaji Name")),
        (AnimeCommandField::KanjiName, text("漢字")),
        (AnimeCommandField::EnglishName, text("English Name")),
        (
            AnimeCommandField::OtherNameList,
            strs(&["other a", "other b"]),
        ),
        (
            AnimeCommandField::ShortNameList,
            strs(&["short1", "short2"]),
        ),
        (AnimeCommandField::SynonymList, strs(&["syn"])),
        (AnimeCommandField::Episodes, int(26)),
        (AnimeCommandField::HighestEpNumber, int(26)),
        (AnimeCommandField::SpecialEpCount, int(2)),
        (AnimeCommandField::AirDate, int(900_000_000)),
        (AnimeCommandField::EndDate, int(900_500_000)),
        (AnimeCommandField::Url, text("http://example.org/x")),
        (AnimeCommandField::PicName, text("pic.jpg")),
        (AnimeCommandField::Rating, int(850)),
        (AnimeCommandField::VoteCount, int(120)),
        (AnimeCommandField::TempRating, int(845)),
        (AnimeCommandField::TempVoteCount, int(30)),
        (AnimeCommandField::AverageReviewRating, int(870)),
        (AnimeCommandField::ReviewCount, int(4)),
        (AnimeCommandField::AwardList, strs(&["award a", "award b"])),
        (AnimeCommandField::Is18Restricted, Value::Bool(true)),
        (AnimeCommandField::AnnId, int(1234)),
        (AnimeCommandField::AllcinemaId, int(5678)),
        (AnimeCommandField::AnimenfoId, text("nfo1")),
        (AnimeCommandField::TagNameList, strs(&["tag a", "tag b"])),
        (AnimeCommandField::TagIdList, ints(&[1, 2])),
        (AnimeCommandField::TagWeightList, ints(&[100, 200])),
        (AnimeCommandField::DateRecordUpdated, int(1_600_000_000)),
        (AnimeCommandField::CharacterIdList, ints(&[1, 2, 3])),
        (AnimeCommandField::SpecialsCount, int(1)),
        (AnimeCommandField::CreditsCount, int(2)),
        (AnimeCommandField::OtherCount, int(3)),
        (AnimeCommandField::TrailerCount, int(4)),
        (AnimeCommandField::ParodyCount, int(5)),
    ]
    .into_iter()
    .collect();
    assert_eq!(reply.fields, expected);
    assert_eq!(
        reply.fields[&AnimeCommandField::DateFlags].as_dateflags(),
        Some(&[DateFlag::StartUnknownDay, DateFlag::EndUnknownDay][..])
    );
}

#[test]
fn default_reply_with_empty_values() {
    // "1|||||||||||||||" with the default mask gives aid 1, empty
    // strings, empty lists and zeros.
    let reply = AnimeReply::decode(&line("1|||||||||||||||"), &default_mask()).unwrap();
    assert_eq!(reply.aid, 1);
    assert_eq!(reply.fields[&AnimeCommandField::Year], text(""));
    assert_eq!(reply.fields[&AnimeCommandField::OtherNameList], strs(&[]));
    assert_eq!(reply.fields[&AnimeCommandField::Episodes], int(0));
    assert_eq!(reply.fields[&AnimeCommandField::ReviewCount], int(0));
    assert_eq!(reply.fields.len(), 16);
}

#[test]
fn mask_without_aid_is_rejected_and_bad_numbers_are_errors() {
    let mut no_aid = AnimeCommandFields::new();
    no_aid.insert(AnimeCommandField::Year);
    assert!(matches!(
        AnimeReply::decode(&line("x"), &no_aid),
        Err(Error::BadData(_))
    ));

    let aid_only = mask([]);
    assert!(matches!(
        AnimeReply::decode(&line("10x"), &aid_only),
        Err(Error::BadData(_))
    ));
    // a reply with fewer values than the mask asks for
    let two = mask([AnimeCommandField::Year]);
    assert!(matches!(
        AnimeReply::decode(&line("1"), &two),
        Err(Error::BadData(_))
    ));
}
