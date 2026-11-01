//! Decodes a recorded FILE reply and checks every value against the fixture
//! `fixtures/anidb-file.fid.toml`, which holds the values the renamer sees for
//! the same file.
//!
//! The reply line was decoded by an independent implementation to the same values,
//! so it is a faithful recording of what the server sends for these fields.

use anidb_udp::file::{request, FileReply};
use anidb_udp::mask::{AnimeField, AnimeFields, FileField, FileFields, Value};
use anidb_udp::wire::parse;
use anidb_udp::ResponseCode;

const FIXTURE: &str = include_str!("fixtures/anidb-file.fid.toml");

/// The data line of `220 FILE`, for the fields the renamer requests.
const REPLY_LINE: &str = "1628637|10914|163945|13184|212018403|4101|447945964|7dafd31d6d028e981d09301c5fbfc654|82100105622e04d5b0a5fa1fe82734f4|37641c2055d9373c5a74eb45ca968676e5f22ac8|438a7b93|very high|Blu-ray|(HE-)AAC|271|H264/AVC|1960|1280x720|mkv|japanese|english|1598||Trinity Seven: Nanatsu no Taizai to Nana Madoushi - 1 - OAD - [BlurayDesuYo](438a7b93).mkv|1|1|2015-2015|OVA||Trinity Seven: Nanatsu no Taizai to Nana Madoushi|トリニティセブン 七つの大罪と七魔道士||トリニティセブン 七つの大罪と七魔道士'TRINITY SEVEN 七人魔法使 OAD|T7 ova|Trinity Seven: The Seven Deadly Sins and the Seven Mages'Trinity Seven and Special Lesson|1|OAD|||BlurayDesuYo|BlurayDesuYo";

/// The fmask fields the renamer requests.
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

/// The amask fields the renamer requests.
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

/// Decodes the recorded line the way `wire::parse` hands it over.
fn decode_recorded() -> FileReply {
    let reply = parse(&format!("ABCDE 220 FILE\n{REPLY_LINE}\n")).unwrap();
    assert_eq!(reply.code, ResponseCode::File);
    FileReply::decode(
        &reply.lines[0],
        &renamer_file_fields(),
        &renamer_anime_fields(),
    )
    .unwrap()
}

#[test]
fn request_carries_the_renamer_masks() {
    let request = request(
        "7dafd31d6d028e981d09301c5fbfc654",
        447_945_964,
        &renamer_file_fields(),
        &renamer_anime_fields(),
    );
    assert_eq!(
        request.encode(None, "T"),
        "FILE ed2k=7dafd31d6d028e981d09301c5fbfc654&size=447945964&fmask=79f8fff100&amask=f2fcf0c0&tag=T"
    );
}

#[test]
fn recorded_reply_decodes_to_every_fixture_value() {
    let fixture: toml::Table = toml::from_str(FIXTURE).unwrap();
    let decoded = decode_recorded();

    let int = |key: &str| fixture[key].as_integer().unwrap();
    let text = |key: &str| fixture[key].as_str().unwrap().to_owned();
    let file = |field: FileField| &decoded.file[&field];

    assert_eq!(decoded.fid, int("fid"));
    assert_eq!(decoded.file.len(), 23);
    assert_eq!(decoded.anime.len(), 17);

    // file
    assert_eq!(file(FileField::Aid), &Value::Int(int("aid")));
    assert_eq!(file(FileField::Eid), &Value::Int(int("eid")));
    assert_eq!(file(FileField::Gid), &Value::Int(int("gid")));
    assert_eq!(file(FileField::MylistId), &Value::Int(int("lid")));
    assert_eq!(file(FileField::State), &Value::Int(int("status")));
    assert_eq!(file(FileField::Size), &Value::Int(int("size")));
    assert_eq!(file(FileField::Ed2k), &Value::Str(text("ed2k")));
    assert_eq!(file(FileField::Md5), &Value::Str(text("md5")));
    assert_eq!(file(FileField::Sha1), &Value::Str(text("sha1")));
    assert_eq!(file(FileField::Crc32), &Value::Str(text("crc32")));
    assert_eq!(file(FileField::Quality), &Value::Str(text("quaility")));
    assert_eq!(file(FileField::Source), &Value::Str(text("source")));
    assert_eq!(
        file(FileField::AudioCodecs),
        &Value::Strs(vec![text("audio_codec")])
    );
    assert_eq!(
        file(FileField::AudioBitrates),
        &Value::Ints(vec![text("audio_bitrate").parse().unwrap()])
    );
    assert_eq!(
        file(FileField::VideoCodec),
        &Value::Str(text("video_codec"))
    );
    assert_eq!(
        file(FileField::VideoBitrate),
        &Value::Int(text("video_bitrate").parse().unwrap())
    );
    assert_eq!(
        file(FileField::VideoResolution),
        &Value::Str(text("resolution"))
    );
    assert_eq!(
        file(FileField::FileExtension),
        &Value::Str(text("filetype"))
    );
    // the renamer joins these lists with `'`, the fixture keeps the joined text
    assert_eq!(
        file(FileField::DubLanguage),
        &Value::Strs(vec![text("lang_dub")])
    );
    assert_eq!(
        file(FileField::SubLanguage),
        &Value::Strs(vec![text("lang_sub")])
    );
    assert_eq!(file(FileField::LengthSeconds), &Value::Int(int("length")));
    assert_eq!(
        file(FileField::Description),
        &Value::Str(text("description"))
    );
    assert_eq!(
        file(FileField::AnidbFilename),
        &Value::Str(text("orginal_name"))
    );

    check_anime(&decoded, &fixture);
}

/// The amask fields, which describe the anime, the episode and the group.
fn check_anime(decoded: &FileReply, fixture: &toml::Table) {
    let int = |key: &str| fixture[key].as_integer().unwrap();
    let text = |key: &str| fixture[key].as_str().unwrap().to_owned();
    let texts = |key: &str| {
        fixture[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let anime = |field: AnimeField| &decoded.anime[&field];

    assert_eq!(
        anime(AnimeField::AnimeTotalEpisodes),
        &Value::Int(int("episode_total"))
    );
    assert_eq!(
        anime(AnimeField::HighestEpisode),
        &Value::Int(int("episode_last"))
    );
    let years: Vec<i64> = fixture["anime_year"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_integer().unwrap())
        .collect();
    assert_eq!(
        anime(AnimeField::Year),
        &Value::Str(format!("{}-{}", years[0], years[1]))
    );
    assert_eq!(anime(AnimeField::Type), &Value::Str(text("anime_type")));
    assert_eq!(
        anime(AnimeField::CategoryList),
        &Value::Strs(texts("anime_category"))
    );
    assert_eq!(
        anime(AnimeField::RomanjiName),
        &Value::Str(text("anime_name_romaji"))
    );
    assert_eq!(
        anime(AnimeField::KanjiName),
        &Value::Str(text("anime_name_kanji"))
    );
    assert_eq!(
        anime(AnimeField::EnglishName),
        &Value::Str(text("anime_name_english"))
    );
    assert_eq!(
        anime(AnimeField::OtherNameList),
        &Value::Strs(texts("anime_name_other"))
    );
    assert_eq!(
        anime(AnimeField::ShortNameList),
        &Value::Strs(texts("anime_name_short"))
    );
    assert_eq!(
        anime(AnimeField::SynonymList),
        &Value::Strs(texts("anime_synonyms"))
    );
    assert_eq!(anime(AnimeField::Epno), &Value::Str(text("episode")));
    assert_eq!(anime(AnimeField::EpName), &Value::Str(text("episode_name")));
    assert_eq!(
        anime(AnimeField::EpRomanjiName),
        &Value::Str(text("episode_name_romaji"))
    );
    assert_eq!(
        anime(AnimeField::EpKanjiName),
        &Value::Str(text("episode_name_kanji"))
    );
    assert_eq!(anime(AnimeField::GroupName), &Value::Str(text("group")));
    assert_eq!(
        anime(AnimeField::GroupShortName),
        &Value::Str(text("group_short"))
    );

    // The renamer derives `version` (v2 for bit 0x04 of the state) and
    // `censored` (empty when neither 0x40 nor 0x80 is set) from the state.
    let state = int("status");
    assert_ne!(state & 0x04, 0);
    assert_eq!(text("version"), "v2");
    assert_eq!(state & 0xc0, 0);
    assert_eq!(text("censored"), "");
}

#[test]
fn short_reply_is_an_error() {
    let line = REPLY_LINE.rsplit_once('|').unwrap().0;
    let reply = parse(&format!("ABCDE 220 FILE\n{line}\n")).unwrap();
    assert!(FileReply::decode(
        &reply.lines[0],
        &renamer_file_fields(),
        &renamer_anime_fields()
    )
    .is_err());
}

#[test]
fn extra_trailing_fields_are_ignored() {
    let reply = parse(&format!("ABCDE 220 FILE\n{REPLY_LINE}|new field\n")).unwrap();
    let decoded = FileReply::decode(
        &reply.lines[0],
        &renamer_file_fields(),
        &renamer_anime_fields(),
    )
    .unwrap();
    assert_eq!(decoded.fid, 1_628_637);
}

#[test]
fn bad_number_is_an_error() {
    let line = REPLY_LINE.replacen("|10914|", "|10x14|", 1);
    let reply = parse(&format!("ABCDE 220 FILE\n{line}\n")).unwrap();
    assert!(FileReply::decode(
        &reply.lines[0],
        &renamer_file_fields(),
        &renamer_anime_fields()
    )
    .is_err());
}
