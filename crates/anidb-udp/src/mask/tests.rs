use super::*;

/// The field list akiba requests for FILE,
/// in the order the reply lists them.
fn renamer_file_fields() -> FileFields {
    [
        FileField::Aid,
        FileField::Eid,
        FileField::Gid,
        FileField::MylistId,
        FileField::State,
        FileField::Size,
        FileField::Ed2k,
        FileField::Md5,
        FileField::Sha1,
        FileField::Crc32,
        FileField::Quality,
        FileField::Source,
        FileField::AudioCodecs,
        FileField::AudioBitrates,
        FileField::VideoCodec,
        FileField::VideoBitrate,
        FileField::VideoResolution,
        FileField::FileExtension,
        FileField::DubLanguage,
        FileField::SubLanguage,
        FileField::LengthSeconds,
        FileField::Description,
        FileField::AnidbFilename,
    ]
    .into_iter()
    .collect()
}

fn renamer_anime_fields() -> AnimeFields {
    [
        AnimeField::AnimeTotalEpisodes,
        AnimeField::HighestEpisode,
        AnimeField::Year,
        AnimeField::Type,
        AnimeField::CategoryList,
        AnimeField::RomanjiName,
        AnimeField::KanjiName,
        AnimeField::EnglishName,
        AnimeField::OtherNameList,
        AnimeField::ShortNameList,
        AnimeField::SynonymList,
        AnimeField::Epno,
        AnimeField::EpName,
        AnimeField::EpRomanjiName,
        AnimeField::EpKanjiName,
        AnimeField::GroupName,
        AnimeField::GroupShortName,
    ]
    .into_iter()
    .collect()
}

#[test]
fn renamer_masks_match_the_reference_hex() {
    // reference values computed independently of the Rust tables
    assert_eq!(renamer_file_fields().to_hex(), "79f8fff100");
    assert_eq!(renamer_anime_fields().to_hex(), "f2fcf0c0");
}

#[test]
fn renamer_fields_come_back_in_table_order() {
    let file: Vec<FileField> = renamer_file_fields().iter().collect();
    assert_eq!(
        file,
        [
            FileField::Aid,
            FileField::Eid,
            FileField::Gid,
            FileField::MylistId,
            FileField::State,
            FileField::Size,
            FileField::Ed2k,
            FileField::Md5,
            FileField::Sha1,
            FileField::Crc32,
            FileField::Quality,
            FileField::Source,
            FileField::AudioCodecs,
            FileField::AudioBitrates,
            FileField::VideoCodec,
            FileField::VideoBitrate,
            FileField::VideoResolution,
            FileField::FileExtension,
            FileField::DubLanguage,
            FileField::SubLanguage,
            FileField::LengthSeconds,
            FileField::Description,
            FileField::AnidbFilename,
        ]
    );
    let anime: Vec<AnimeField> = renamer_anime_fields().iter().collect();
    assert_eq!(anime.first(), Some(&AnimeField::AnimeTotalEpisodes));
    assert_eq!(anime.last(), Some(&AnimeField::GroupShortName));
    assert_eq!(anime.len(), 17);
}

#[test]
fn empty_and_single_field_masks() {
    assert_eq!(FileFields::new().to_hex(), "0000000000");
    assert_eq!(AnimeFields::new().to_hex(), "00000000");
    let mut size_only = FileFields::new();
    size_only.insert(FileField::Size);
    size_only.insert(FileField::Size);
    assert_eq!(size_only.to_hex(), "0080000000");
    assert_eq!(size_only.len(), 1);
    assert!(size_only.contains(FileField::Size));
    assert!(!size_only.contains(FileField::Ed2k));
}

#[test]
fn tables_have_one_row_per_field_and_fit_their_bytes() {
    assert_eq!(FileField::TABLE.len(), 34);
    assert_eq!(AnimeField::TABLE.len(), 22);
    for spec in FileField::TABLE {
        assert!(FileField::BYTES.contains(&spec.byte), "{}", spec.name);
    }
    for spec in AnimeField::TABLE {
        assert!(AnimeField::BYTES.contains(&spec.byte), "{}", spec.name);
    }
}

#[test]
fn table_rows_are_unique() {
    fn check<F: Field>() {
        let mut bits = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        let mut fields = std::collections::BTreeSet::new();
        for spec in F::TABLE {
            assert!(
                bits.insert((spec.byte, spec.bit)),
                "bit reused: {}",
                spec.name
            );
            assert!(names.insert(spec.name), "name reused: {}", spec.name);
            assert!(fields.insert(spec.field), "field repeated: {}", spec.name);
        }
    }
    check::<FileField>();
    check::<AnimeField>();
}

#[test]
fn decode_follows_mask_order_and_ignores_trailing_fields() {
    let mut set = FileFields::new();
    set.insert(FileField::Ed2k);
    set.insert(FileField::Aid);
    let values = ["10914", "abc", "extra"];
    let decoded = decode_fields(set, &values[..2]).unwrap();
    assert_eq!(decoded[&FileField::Aid], Value::Int(10914));
    assert_eq!(decoded[&FileField::Ed2k], Value::Str("abc".into()));

    let decoded = decode_fields(set, &values).unwrap();
    assert_eq!(decoded.len(), 2);
}

#[test]
fn decode_turns_line_breaks_into_newlines_in_text_fields() {
    let mut set = FileFields::new();
    set.insert(FileField::Description);
    let decoded = decode_fields(set, &["first<br />second"]).unwrap();
    assert_eq!(
        decoded[&FileField::Description],
        Value::Str("first\nsecond".into())
    );
    let mut set = AnimeFields::new();
    set.insert(AnimeField::RomanjiName);
    set.insert(AnimeField::SynonymList);
    let decoded = decode_fields(set, &["Name<br />Two", "a<br />b'c"]).unwrap();
    assert_eq!(
        decoded[&AnimeField::RomanjiName],
        Value::Str("Name\nTwo".into())
    );
    assert_eq!(
        decoded[&AnimeField::SynonymList],
        Value::Strs(vec!["a\nb".into(), "c".into()])
    );
}

#[test]
fn decode_rejects_short_replies_and_bad_numbers() {
    let mut set = FileFields::new();
    set.insert(FileField::Aid);
    set.insert(FileField::Size);
    assert!(matches!(decode_fields(set, &["1"]), Err(Error::BadData(_))));
    assert!(matches!(
        decode_fields(set, &["1", "x"]),
        Err(Error::BadData(_))
    ));
}
