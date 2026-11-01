//! The ANIME command's amask fields with their table, and the decoded reply.
//! The mask builders and `AnimeReply::decode` are in [`crate::anime`].

use std::collections::BTreeMap;
use std::ops::Range;

use super::mask::{Field, FieldSet, Kind, Spec, Value, INT_CSV, INT_LIST, STR_CSV, STR_LIST};

/// A field of the ANIME amask: a property of the anime. The names are the ones
/// of the ANIME reply fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum AnimeCommandField {
    /// Anime id. Always requested.
    Aid,
    /// Date flags of the anime's air dates.
    DateFlags,
    /// Year, or `start-end`.
    Year,
    /// Anime type, such as "OVA".
    Type,
    /// Related anime ids.
    RelatedAnime,
    /// Relation types, one per related anime.
    RelatedTypes,
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
    /// Number of episodes.
    Episodes,
    /// Highest episode number.
    HighestEpNumber,
    /// Number of special episodes.
    SpecialEpCount,
    /// Air date, unix time.
    AirDate,
    /// End date, unix time.
    EndDate,
    /// Official website.
    Url,
    /// Picture file name.
    PicName,
    /// Rating, times 100.
    Rating,
    /// Number of votes.
    VoteCount,
    /// Temporary rating, times 100.
    TempRating,
    /// Number of temporary votes.
    TempVoteCount,
    /// Average review rating, times 100.
    AverageReviewRating,
    /// Number of reviews.
    ReviewCount,
    /// Awards.
    AwardList,
    /// Whether the anime is restricted to adults.
    Is18Restricted,
    /// Anime News Network id.
    AnnId,
    /// Allcinema id.
    AllcinemaId,
    /// `AnimeNfo` id.
    AnimenfoId,
    /// Tag names, comma separated.
    TagNameList,
    /// Tag ids, comma separated.
    TagIdList,
    /// Tag weights, comma separated.
    TagWeightList,
    /// Date the `AniDB` record was last updated, unix time.
    DateRecordUpdated,
    /// Character ids, comma separated.
    CharacterIdList,
    /// Number of specials.
    SpecialsCount,
    /// Number of credits (opening and ending) episodes.
    CreditsCount,
    /// Number of other episodes.
    OtherCount,
    /// Number of trailers.
    TrailerCount,
    /// Number of parodies.
    ParodyCount,
}

impl Field for AnimeCommandField {
    const BYTES: Range<u8> = 0..7;
    const TABLE: &'static [Spec<Self>] = &[
        // byte 0 of the amask: 0x80 aid, 0x40 dateflags, 0x20 year, 0x10 type,
        // 0x08 related anime, 0x04 related types, 0x02 and 0x01 retired.
        Spec::new(Self::Aid, 0, 0x80, "aid", Kind::Int),
        Spec::new(Self::DateFlags, 0, 0x40, "dateflags", Kind::DateFlags),
        Spec::new(Self::Year, 0, 0x20, "year", Kind::Str),
        Spec::new(Self::Type, 0, 0x10, "type", Kind::Str),
        Spec::new(Self::RelatedAnime, 0, 0x08, "related_anime", INT_LIST),
        Spec::new(Self::RelatedTypes, 0, 0x04, "related_types", INT_LIST),
        // byte 1 of the amask: 0x80 romaji name, 0x40 kanji name, 0x20 english
        // name, 0x10 other names, 0x08 short names, 0x04 synonyms, 0x02 and 0x01
        // retired.
        Spec::new(Self::RomanjiName, 1, 0x80, "romanji_name", Kind::Str),
        Spec::new(Self::KanjiName, 1, 0x40, "kanji_name", Kind::Str),
        Spec::new(Self::EnglishName, 1, 0x20, "english_name", Kind::Str),
        Spec::new(Self::OtherNameList, 1, 0x10, "other_name_list", STR_LIST),
        Spec::new(Self::ShortNameList, 1, 0x08, "short_name_list", STR_LIST),
        Spec::new(Self::SynonymList, 1, 0x04, "synonym_list", STR_LIST),
        // byte 2 of the amask: 0x80 episodes, 0x40 highest episode number,
        // 0x20 special episode count, 0x10 air date, 0x08 end date, 0x04 url,
        // 0x02 picname, 0x01 retired.
        Spec::new(Self::Episodes, 2, 0x80, "episodes", Kind::Int),
        Spec::new(
            Self::HighestEpNumber,
            2,
            0x40,
            "highest_ep_number",
            Kind::Int,
        ),
        Spec::new(Self::SpecialEpCount, 2, 0x20, "special_ep_count", Kind::Int),
        Spec::new(Self::AirDate, 2, 0x10, "air_date", Kind::Int),
        Spec::new(Self::EndDate, 2, 0x08, "end_date", Kind::Int),
        Spec::new(Self::Url, 2, 0x04, "url", Kind::Str),
        Spec::new(Self::PicName, 2, 0x02, "picname", Kind::Str),
        // byte 3 of the amask: 0x80 rating, 0x40 vote count, 0x20 temporary
        // rating, 0x10 temporary vote count, 0x08 average review rating, 0x04
        // review count, 0x02 award list, 0x01 is 18+ restricted.
        Spec::new(Self::Rating, 3, 0x80, "rating", Kind::Int),
        Spec::new(Self::VoteCount, 3, 0x40, "vote_count", Kind::Int),
        Spec::new(Self::TempRating, 3, 0x20, "temp_rating", Kind::Int),
        Spec::new(Self::TempVoteCount, 3, 0x10, "temp_vote_count", Kind::Int),
        Spec::new(
            Self::AverageReviewRating,
            3,
            0x08,
            "average_review_rating",
            Kind::Int,
        ),
        Spec::new(Self::ReviewCount, 3, 0x04, "review_count", Kind::Int),
        Spec::new(Self::AwardList, 3, 0x02, "award_list", STR_LIST),
        Spec::new(
            Self::Is18Restricted,
            3,
            0x01,
            "is_18_restricted",
            Kind::Bool,
        ),
        // byte 4 of the amask: 0x80 retired, 0x40 ANN id, 0x20 allcinema id,
        // 0x10 AnimeNfo id, 0x08 tag names, 0x04 tag ids, 0x02 tag weights, 0x01
        // date of the record update.
        Spec::new(Self::AnnId, 4, 0x40, "ann_id", Kind::Int),
        Spec::new(Self::AllcinemaId, 4, 0x20, "allcinema_id", Kind::Int),
        Spec::new(Self::AnimenfoId, 4, 0x10, "animenfo_id", Kind::Str),
        Spec::new(Self::TagNameList, 4, 0x08, "tag_name_list", STR_CSV),
        Spec::new(Self::TagIdList, 4, 0x04, "tag_id_list", INT_CSV),
        Spec::new(Self::TagWeightList, 4, 0x02, "tag_weight_list", INT_CSV),
        Spec::new(
            Self::DateRecordUpdated,
            4,
            0x01,
            "date_record_updated",
            Kind::Int,
        ),
        // byte 5 of the amask: 0x80 character id list, 0x40 to 0x10 retired,
        // 0x08 to 0x01 unused.
        Spec::new(Self::CharacterIdList, 5, 0x80, "character_id_list", INT_CSV),
        // byte 6 of the amask: 0x80 specials count, 0x40 credits count, 0x20
        // other count, 0x10 trailer count, 0x08 parody count, 0x04 to 0x01
        // unused.
        Spec::new(Self::SpecialsCount, 6, 0x80, "specials_count", Kind::Int),
        Spec::new(Self::CreditsCount, 6, 0x40, "credits_count", Kind::Int),
        Spec::new(Self::OtherCount, 6, 0x20, "other_count", Kind::Int),
        Spec::new(Self::TrailerCount, 6, 0x10, "trailer_count", Kind::Int),
        Spec::new(Self::ParodyCount, 6, 0x08, "parody_count", Kind::Int),
    ];
}

/// The fields an ANIME command asks for.
pub type AnimeCommandFields = FieldSet<AnimeCommandField>;

/// A decoded ANIME reply: the anime id, and every field the mask asked for. The
/// fields are kept as decoded values, see [`Value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimeReply {
    /// The anime id, always requested.
    pub aid: i64,
    /// The requested fields, `aid` among them.
    pub fields: BTreeMap<AnimeCommandField, Value>,
}
