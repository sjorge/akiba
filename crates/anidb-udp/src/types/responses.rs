//! The typed results of the commands whose replies are a fixed record: the
//! structs, the code enums and their `from_code`. The `parse` of each is in
//! [`crate::responses`].

use super::dateflag::DateFlag;
use crate::codes::ResponseCode;

/// One part of an ANIMEDESC reply. A description is longer than one packet, so
/// it comes in zero based parts. Join them with [`join_description`](crate::responses::join_description).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimeDesc {
    /// The zero based number of this part.
    pub current_part: i64,
    /// The number of parts the description has.
    pub max_parts: i64,
    /// The text of this part, raw: `<br />` is not turned into a newline yet.
    pub desc: String,
}

/// One line of a CALENDAR reply: `aid|start date|date flags`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEntry {
    /// Anime id.
    pub aid: i64,
    /// Start date, unix time.
    pub startdate: i64,
    /// The unknown parts of the dates.
    pub dateflags: Vec<DateFlag>,
}

/// One anime a character appears in, from the CHARACTER reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterAnime {
    /// Anime id.
    pub aid: i64,
    /// Appearance: 0 appears in, 1 cameo, 2 main character, 3 secondary cast.
    pub appearance: i64,
    /// Creator id of the seiyuu, 0 when there is none.
    pub creatorid: i64,
    /// Whether the seiyuu is the main one.
    pub is_main_seiyuu: bool,
}

/// A CHARACTER reply, `charid|name kanji|name transcription|pic|anime
/// blocks|episode list|last update|type|gender`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Character {
    /// Character id.
    pub charid: i64,
    /// Name in kanji.
    pub character_name_kanji: String,
    /// Name transcribed.
    pub character_name_transc: String,
    /// Picture file name.
    pub pic: String,
    /// The anime the character appears in.
    pub anime_blocks: Vec<CharacterAnime>,
    /// The episode ids the character appears in. Empty when no episodes were
    /// added.
    pub episode_list: Vec<i64>,
    /// Last update, unix time.
    pub last_update_date: i64,
    /// The raw type code. `AniDB` may change the codes, so it is not decoded.
    pub character_type: i64,
    /// The raw gender code, as text.
    pub gender: String,
}

/// The type of a creator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatorType {
    /// 1: a person.
    Person,
    /// 2: a company.
    Company,
    /// 3: a collaboration.
    Collaboration,
}

impl CreatorType {
    /// The type for a code, `None` for codes this client does not know.
    #[must_use]
    pub fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            1 => Self::Person,
            2 => Self::Company,
            3 => Self::Collaboration,
            _ => return None,
        })
    }
}

/// A CREATOR reply, `creatorid|name kanji|name transcription|type|pic|url
/// english|url japanese|wiki url english|wiki url japanese|last update`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creator {
    /// Creator id.
    pub creatorid: i64,
    /// Name in kanji.
    pub creator_name_kanji: String,
    /// Name transcribed.
    pub creator_name_transcription: String,
    /// Type, `None` for an unknown code.
    pub creator_type: Option<CreatorType>,
    /// Picture file name.
    pub pic_name: String,
    /// Website in English.
    pub url_english: String,
    /// Website in Japanese.
    pub url_japanese: String,
    /// Wikipedia page in English.
    pub wiki_url_english: String,
    /// Wikipedia page in Japanese.
    pub wiki_url_japanese: String,
    /// Last update, unix time.
    pub last_update_date: i64,
}

/// The type of an episode. Regular episodes are type 1, the others carry the
/// prefix of their `epno` (`S`, `C`, `T`, `P`, `O`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeType {
    /// 1: a regular episode, no prefix.
    Regular,
    /// 2: a special, `S`.
    Special,
    /// 3: credits, `C`.
    Credits,
    /// 4: a trailer, `T`.
    Trailer,
    /// 5: a parody, `P`.
    Parody,
    /// 6: other, `O`.
    Other,
}

impl EpisodeType {
    /// The type for a code, `None` for codes this client does not know.
    #[must_use]
    pub fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            1 => Self::Regular,
            2 => Self::Special,
            3 => Self::Credits,
            4 => Self::Trailer,
            5 => Self::Parody,
            6 => Self::Other,
            _ => return None,
        })
    }
}

/// An EPISODE reply, `eid|aid|length|rating|votes|epno|eng|romaji|kanji|aired|type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    /// Episode id.
    pub eid: i64,
    /// Anime id.
    pub aid: i64,
    /// Length in minutes.
    pub length: i64,
    /// Rating, times 100.
    pub rating: i64,
    /// Number of votes.
    pub votes: i64,
    /// Episode number, with the prefix of the type for specials. Regular
    /// episode numbers are padded.
    pub epno: String,
    /// English title.
    pub eng: String,
    /// Romaji title.
    pub romaji: String,
    /// Kanji title.
    pub kanji: String,
    /// Air date, unix time.
    pub aired: i64,
    /// The episode type, `None` for an unknown code.
    pub episode_type: Option<EpisodeType>,
}

/// The type of a relation between two groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationType {
    /// 1: participant in.
    ParticipantIn,
    /// 2: parent of.
    ParentOf,
    /// 4: merged from.
    MergedFrom,
    /// 5: now known as.
    NowKnownAs,
    /// 6: other.
    Other,
    /// 105: formerly.
    Formerly,
}

impl RelationType {
    /// The type for a code, `None` for codes this client does not know.
    #[must_use]
    pub fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            1 => Self::ParticipantIn,
            2 => Self::ParentOf,
            4 => Self::MergedFrom,
            5 => Self::NowKnownAs,
            6 => Self::Other,
            105 => Self::Formerly,
            _ => return None,
        })
    }
}

/// A relation of a group to another group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRelation {
    /// The other group's id.
    pub othergroupid: i64,
    /// The relation, `None` for an unknown code.
    pub relationtype: Option<RelationType>,
}

/// A GROUP reply: `gid|rating|votes|acount|fcount|name|short|irc channel|irc
/// server|url|picname|founded|disbanded|dateflags|last release|last
/// activity|group relations`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// Group id.
    pub gid: i64,
    /// Rating, times 100.
    pub rating: i64,
    /// Number of votes.
    pub votes: i64,
    /// Number of anime.
    pub acount: i64,
    /// Number of files.
    pub fcount: i64,
    /// Name.
    pub name: String,
    /// Short name.
    pub short: String,
    /// IRC channel.
    pub irc_channel: String,
    /// IRC server.
    pub irc_server: String,
    /// Website.
    pub url: String,
    /// Picture file name.
    pub picname: String,
    /// Founding date, unix time.
    pub foundeddate: i64,
    /// Disbanding date, unix time.
    pub disbandeddate: i64,
    /// The unknown parts of the dates.
    pub dateflags: Vec<DateFlag>,
    /// Date of the last release, unix time.
    pub lastreleasedate: i64,
    /// Date of the last file added to `AniDB`, unix time.
    pub lastactivitydate: i64,
    /// Relations to other groups. Empty when there are none.
    pub grouprelations: Vec<GroupRelation>,
}

/// The completion state of a group for an anime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionState {
    /// 1: ongoing.
    Ongoing,
    /// 2: stalled.
    Stalled,
    /// 3: complete.
    Complete,
    /// 4: dropped.
    Dropped,
    /// 5: finished.
    Finished,
    /// 6: specials only.
    SpecialsOnly,
}

impl CompletionState {
    /// The state for a code, `None` for codes this client does not know.
    #[must_use]
    pub fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            1 => Self::Ongoing,
            2 => Self::Stalled,
            3 => Self::Complete,
            4 => Self::Dropped,
            5 => Self::Finished,
            6 => Self::SpecialsOnly,
            _ => return None,
        })
    }
}

/// One line of a GROUPSTATUS reply: `gid|group name|completion state|last
/// episode number|rating|votes|episode range`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupStatusEntry {
    /// Group id.
    pub gid: i64,
    /// Group name.
    pub group_name: String,
    /// Completion state, `None` for an unknown code.
    pub completion_state: Option<CompletionState>,
    /// Last episode number released.
    pub last_episode_number: i64,
    /// Rating, times 100.
    pub rating: i64,
    /// Number of votes.
    pub votes: i64,
    /// The episodes the group released, as text such as `1-12`.
    pub episode_range: String,
}

/// An UPDATED reply, `entity|total count|last update date|aid list`. The entity
/// is always 1 and is checked, not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Updated {
    /// Number of anime updated, which can be more than the list holds. The list
    /// holds at most 200 ids, so a total above 200 means not all were retrieved.
    pub total_count: i64,
    /// Time of the last update in the list, unix time.
    pub last_update_date: i64,
    /// The anime ids updated.
    pub updated_aids: Vec<i64>,
}

/// The statistics of the mylist, from a MYLISTSTATS reply. All fields are
/// integers, as the reply sends them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MylistStats {
    /// Animes in the mylist.
    pub anime: i64,
    /// Episodes in the mylist.
    pub eps: i64,
    /// Files in the mylist.
    pub files: i64,
    /// Size of the files, in bytes.
    pub size_of_files: i64,
    /// Animes added.
    pub added_animes: i64,
    /// Episodes added.
    pub added_eps: i64,
    /// Files added.
    pub added_files: i64,
    /// Groups added.
    pub added_groups: i64,
    /// Leech percentage.
    pub leech_perc: i64,
    /// Glory percentage.
    pub glory_perc: i64,
    /// Viewed percentage of the database.
    pub viewed_perc: i64,
    /// Mylist percentage of the database.
    pub mylist_perc: i64,
    /// Viewed percentage of the mylist.
    pub viewed_mylist_perc: i64,
    /// Number of viewed episodes.
    pub viewed_nr_eps: i64,
    /// Number of votes.
    pub votes: i64,
    /// Number of reviews.
    pub reviews: i64,
    /// Viewed length, in minutes.
    pub views_in_min: i64,
}

/// The type of a vote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteType {
    /// 1: anime.
    Anime,
    /// 2: temporary anime vote.
    AnimeTmp,
    /// 3: group.
    Group,
    /// 6: episode.
    Episode,
}

impl VoteType {
    /// The type for a code, `None` for codes this client does not know.
    #[must_use]
    pub fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            1 => Self::Anime,
            2 => Self::AnimeTmp,
            3 => Self::Group,
            6 => Self::Episode,
            _ => return None,
        })
    }
}

/// The action a VOTE reply reports, from its code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteAction {
    /// 260: voted.
    Voted,
    /// 261: a vote was already there.
    VoteFound,
    /// 262: a vote was updated, the value is the old one.
    VoteUpdated,
    /// 263: a vote was revoked.
    VoteRevoked,
}

impl VoteAction {
    /// The action for a reply code, `None` for any other code.
    #[must_use]
    pub fn from_code(code: ResponseCode) -> Option<Self> {
        Some(match u16::from(code) {
            260 => Self::Voted,
            261 => Self::VoteFound,
            262 => Self::VoteUpdated,
            263 => Self::VoteRevoked,
            _ => return None,
        })
    }
}

/// The vote record of a VOTE reply: `entity name|vote value|vote type|entity
/// id`. Error replies 360, 363 and 364 carry the same record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteRecord {
    /// Name of the voted entity.
    pub name: String,
    /// The vote value: negative when revoked, 100 to 1000 for a vote.
    pub vote_value: i64,
    /// The type of the entity, `None` for an unknown code.
    pub vote_type: Option<VoteType>,
    /// Id of the voted entity.
    pub id: i64,
}

/// A successful VOTE reply: the action of its code, and the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteResult {
    /// What the server did.
    pub action: VoteAction,
    /// The vote the reply is about.
    pub vote: VoteRecord,
}

/// A RANDOMANIME reply. The spec says "see ANIME", and ANIME without an amask
/// replies the 19 fields below, ending at the category list. A reply with more
/// trailing fields is accepted and the extra ones are ignored. The anime id is an
/// integer here, as all ids are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomAnime {
    /// Anime id.
    pub aid: i64,
    /// Number of episodes.
    pub episodes: i64,
    /// Highest episode number.
    pub highest_ep_number: i64,
    /// Number of special episodes.
    pub special_ep_count: i64,
    /// Rating, times 100.
    pub rating: i64,
    /// Number of votes.
    pub vote_count: i64,
    /// Temporary rating, times 100.
    pub temp_rating: i64,
    /// Number of temporary votes.
    pub temp_vote_count: i64,
    /// Average review rating, times 100.
    pub average_review_rating: i64,
    /// Number of reviews.
    pub review_count: i64,
    /// Year, or `start-end`.
    pub year: String,
    /// Anime type, such as "OVA".
    pub anime_type: String,
    /// Romaji name.
    pub romanji_name: String,
    /// Kanji name.
    pub kanji_name: String,
    /// English name.
    pub english_name: String,
    /// Other names.
    pub other_name_list: Vec<String>,
    /// Short names.
    pub short_name_list: Vec<String>,
    /// Synonyms.
    pub synonym_list: Vec<String>,
    /// Categories.
    pub categories: Vec<String>,
}

/// A USER reply, `uid|username`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserId {
    /// User id.
    pub uid: i64,
    /// User name.
    pub username: String,
}
