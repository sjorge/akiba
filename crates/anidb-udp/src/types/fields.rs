//! The fields of the FILE command, one enum for the fmask and one for the
//! amask, and their tables (the mask bit, name and kind of every field).

use std::ops::Range;

use super::mask::{Field, Kind, Spec, INT_LIST, STR_CSV, STR_LIST};

/// A field of the fmask (bytes 0 to 4): a property of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum FileField {
    /// Anime id.
    Aid,
    /// Episode id.
    Eid,
    /// Group id.
    Gid,
    /// Mylist entry id.
    MylistId,
    /// Other episodes, raw text: episode ids, each followed by the percentage of
    /// that episode the file covers (the spec does not give the separators).
    OtherEpisodes,
    /// Whether the file is deprecated.
    IsDeprecated,
    /// File state bit field.
    State,
    /// Size in bytes.
    Size,
    /// ed2k hash.
    Ed2k,
    /// MD5 hash.
    Md5,
    /// SHA1 hash.
    Sha1,
    /// CRC32 checksum.
    Crc32,
    /// Video colour depth.
    VideoColorDepth,
    /// Quality, such as "very high".
    Quality,
    /// Source, such as "Blu-ray".
    Source,
    /// Audio codecs.
    AudioCodecs,
    /// Audio bitrates.
    AudioBitrates,
    /// Video codec.
    VideoCodec,
    /// Video bitrate.
    VideoBitrate,
    /// Video resolution.
    VideoResolution,
    /// File extension.
    FileExtension,
    /// Dub languages.
    DubLanguage,
    /// Subtitle languages.
    SubLanguage,
    /// Length in seconds.
    LengthSeconds,
    /// Description.
    Description,
    /// Air date.
    AiredDate,
    /// Original file name on `AniDB`.
    AnidbFilename,
    /// Mylist state.
    MylistState,
    /// Mylist file state.
    MylistFilestate,
    /// Mylist viewed flag.
    MylistViewed,
    /// Mylist view date.
    MylistViewdate,
    /// Mylist storage.
    MylistStorage,
    /// Mylist source.
    MylistSource,
    /// Mylist remarks.
    MylistOther,
}

/// A field of the amask (bytes 5 to 8): a property of the anime, the episode or
/// the group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum AnimeField {
    /// Total number of episodes.
    AnimeTotalEpisodes,
    /// Highest episode number.
    HighestEpisode,
    /// Year, or `start-end`.
    Year,
    /// Anime type, such as "OVA".
    Type,
    /// Related anime ids.
    RelatedAidList,
    /// Relation types, one per related anime.
    RelatedAidType,
    /// Categories, comma separated.
    CategoryList,
    /// Romaji name.
    RomanjiName,
    /// Kanji name.
    KanjiName,
    /// English name.
    EnglishName,
    /// Other names.
    OtherNameList,
    /// Short names.
    ShortNameList,
    /// Synonyms.
    SynonymList,
    /// Episode number, as text such as "S1".
    Epno,
    /// Episode name.
    EpName,
    /// Episode romaji name.
    EpRomanjiName,
    /// Episode kanji name.
    EpKanjiName,
    /// Episode rating.
    EpRating,
    /// Episode vote count.
    EpVoteCount,
    /// Group name.
    GroupName,
    /// Group short name.
    GroupShortName,
    /// Date the `AniDB` record was last updated.
    AidRecordUpdated,
}

impl Field for FileField {
    const BYTES: Range<u8> = 0..5;
    const TABLE: &'static [Spec<Self>] = &[
        // byte 0 of the fmask: 0x80 unused, 0x40 aid, 0x20 eid, 0x10 gid,
        // 0x08 mylist id, 0x04 other episodes, 0x02 is deprecated, 0x01 state.
        Spec::new(Self::Aid, 0, 0x40, "aid", Kind::Int),
        Spec::new(Self::Eid, 0, 0x20, "eid", Kind::Int),
        Spec::new(Self::Gid, 0, 0x10, "gid", Kind::Int),
        Spec::new(Self::MylistId, 0, 0x08, "mylist_id", Kind::Int),
        Spec::new(Self::OtherEpisodes, 0, 0x04, "other_episodes", Kind::Str),
        Spec::new(Self::IsDeprecated, 0, 0x02, "is_deprecated", Kind::Bool),
        Spec::new(Self::State, 0, 0x01, "state", Kind::Int),
        // byte 1 of the fmask: 0x80 size, 0x40 ed2k, 0x20 md5, 0x10 sha1,
        // 0x08 crc32, 0x04 unused, 0x02 video colour depth, 0x01 reserved.
        Spec::new(Self::Size, 1, 0x80, "size", Kind::Int),
        Spec::new(Self::Ed2k, 1, 0x40, "ed2k", Kind::Str),
        Spec::new(Self::Md5, 1, 0x20, "md5", Kind::Str),
        Spec::new(Self::Sha1, 1, 0x10, "sha1", Kind::Str),
        Spec::new(Self::Crc32, 1, 0x08, "crc32", Kind::Str),
        Spec::new(
            Self::VideoColorDepth,
            1,
            0x02,
            "video_color_depth",
            Kind::Str,
        ),
        // byte 2 of the fmask: 0x80 quality, 0x40 source, 0x20 audio codec list,
        // 0x10 audio bitrate list, 0x08 video codec, 0x04 video bitrate,
        // 0x02 video resolution, 0x01 file type (extension).
        Spec::new(Self::Quality, 2, 0x80, "quality", Kind::Str),
        Spec::new(Self::Source, 2, 0x40, "source", Kind::Str),
        Spec::new(Self::AudioCodecs, 2, 0x20, "audio_codecs", STR_LIST),
        Spec::new(Self::AudioBitrates, 2, 0x10, "audio_bitrates", INT_LIST),
        Spec::new(Self::VideoCodec, 2, 0x08, "video_codec", Kind::Str),
        Spec::new(Self::VideoBitrate, 2, 0x04, "video_bitrate", Kind::Int),
        Spec::new(
            Self::VideoResolution,
            2,
            0x02,
            "video_resolution",
            Kind::Str,
        ),
        Spec::new(Self::FileExtension, 2, 0x01, "file_extension", Kind::Str),
        // byte 3 of the fmask: 0x80 dub language, 0x40 sub language,
        // 0x20 length in seconds, 0x10 description, 0x08 aired date,
        // 0x04 unused, 0x02 unused, 0x01 anidb file name.
        Spec::new(Self::DubLanguage, 3, 0x80, "dub_language", STR_LIST),
        Spec::new(Self::SubLanguage, 3, 0x40, "sub_language", STR_LIST),
        Spec::new(Self::LengthSeconds, 3, 0x20, "length_seconds", Kind::Int),
        Spec::new(Self::Description, 3, 0x10, "description", Kind::Str),
        Spec::new(Self::AiredDate, 3, 0x08, "aired_date", Kind::Int),
        Spec::new(Self::AnidbFilename, 3, 0x01, "anidb_filename", Kind::Str),
        // byte 4 of the fmask: 0x80 mylist state, 0x40 mylist filestate,
        // 0x20 mylist viewed, 0x10 mylist viewdate, 0x08 mylist storage,
        // 0x04 mylist source, 0x02 mylist other, 0x01 unused.
        Spec::new(Self::MylistState, 4, 0x80, "mylist_state", Kind::Int),
        Spec::new(
            Self::MylistFilestate,
            4,
            0x40,
            "mylist_filestate",
            Kind::Int,
        ),
        Spec::new(Self::MylistViewed, 4, 0x20, "mylist_viewed", Kind::Int),
        Spec::new(Self::MylistViewdate, 4, 0x10, "mylist_viewdate", Kind::Int),
        Spec::new(Self::MylistStorage, 4, 0x08, "mylist_storage", Kind::Str),
        Spec::new(Self::MylistSource, 4, 0x04, "mylist_source", Kind::Str),
        Spec::new(Self::MylistOther, 4, 0x02, "mylist_other", Kind::Str),
    ];
}

impl Field for AnimeField {
    const BYTES: Range<u8> = 5..9;
    const TABLE: &'static [Spec<Self>] = &[
        // byte 5 of the amask: 0x80 anime total episodes, 0x40 highest episode
        // number, 0x20 year, 0x10 type, 0x08 related aid list, 0x04 related aid
        // type, 0x02 category list, 0x01 reserved.
        Spec::new(
            Self::AnimeTotalEpisodes,
            5,
            0x80,
            "anime_total_episodes",
            Kind::Int,
        ),
        Spec::new(Self::HighestEpisode, 5, 0x40, "highest_episode", Kind::Int),
        Spec::new(Self::Year, 5, 0x20, "year", Kind::Str),
        Spec::new(Self::Type, 5, 0x10, "type", Kind::Str),
        Spec::new(Self::RelatedAidList, 5, 0x08, "related_aid_list", INT_LIST),
        Spec::new(Self::RelatedAidType, 5, 0x04, "related_aid_type", INT_LIST),
        Spec::new(Self::CategoryList, 5, 0x02, "category_list", STR_CSV),
        // byte 6 of the amask: 0x80 romaji name, 0x40 kanji name, 0x20 english
        // name, 0x10 other name list, 0x08 short name list, 0x04 synonym list,
        // 0x02 and 0x01 retired.
        Spec::new(Self::RomanjiName, 6, 0x80, "romanji_name", Kind::Str),
        Spec::new(Self::KanjiName, 6, 0x40, "kanji_name", Kind::Str),
        Spec::new(Self::EnglishName, 6, 0x20, "english_name", Kind::Str),
        Spec::new(Self::OtherNameList, 6, 0x10, "other_name_list", STR_LIST),
        Spec::new(Self::ShortNameList, 6, 0x08, "short_name_list", STR_LIST),
        Spec::new(Self::SynonymList, 6, 0x04, "synonym_list", STR_LIST),
        // byte 7 of the amask: 0x80 epno, 0x40 episode name, 0x20 episode romaji
        // name, 0x10 episode kanji name, 0x08 episode rating, 0x04 episode vote
        // count, 0x02 and 0x01 unused.
        Spec::new(Self::Epno, 7, 0x80, "epno", Kind::Str),
        Spec::new(Self::EpName, 7, 0x40, "ep_name", Kind::Str),
        Spec::new(Self::EpRomanjiName, 7, 0x20, "ep_romanji_name", Kind::Str),
        Spec::new(Self::EpKanjiName, 7, 0x10, "ep_kanji_name", Kind::Str),
        Spec::new(Self::EpRating, 7, 0x08, "ep_rating", Kind::Int),
        Spec::new(Self::EpVoteCount, 7, 0x04, "ep_vote_count", Kind::Int),
        // byte 8 of the amask: 0x80 group name, 0x40 group short name, 0x20 to
        // 0x02 unused, 0x01 date of the aid record update.
        Spec::new(Self::GroupName, 8, 0x80, "group_name", Kind::Str),
        Spec::new(Self::GroupShortName, 8, 0x40, "group_short_name", Kind::Str),
        Spec::new(
            Self::AidRecordUpdated,
            8,
            0x01,
            "aid_record_updated",
            Kind::Int,
        ),
    ];
}
