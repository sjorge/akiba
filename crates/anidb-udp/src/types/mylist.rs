//! The typed result of the MYLIST command: the entry, the by anime summary and
//! the state enums. The `parse` of each is in [`crate::mylist`].

/// The storage state of a mylist entry (`state` in the reply).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MylistState {
    /// 0: unknown.
    Unknown,
    /// 1: internal storage (hdd, not shared).
    InternalStorage,
    /// 2: external storage (cd or dvd).
    ExternalStorage,
    /// 3: deleted.
    Deleted,
    /// 4: remote storage (nas or cloud).
    RemoteStorage,
}

impl MylistState {
    /// The state for a code, `None` for codes this client does not know.
    #[must_use]
    pub fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            0 => Self::Unknown,
            1 => Self::InternalStorage,
            2 => Self::ExternalStorage,
            3 => Self::Deleted,
            4 => Self::RemoteStorage,
            _ => return None,
        })
    }
}

/// The file state of a mylist entry (`filestate` in the reply).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MylistFileState {
    /// 0: normal or original.
    NormalOriginal,
    /// 1: corrupted version or invalid crc.
    CorruptedInvalidCrc,
    /// 2: self edited.
    SelfEdited,
    /// 10: self ripped.
    SelfRipped,
    /// 11: on dvd.
    OnDvd,
    /// 12: on vhs.
    OnVhs,
    /// 13: on tv.
    OnTv,
    /// 14: in theaters.
    InTheaters,
    /// 15: streamed.
    Streamed,
    /// 100: other.
    Other,
}

impl MylistFileState {
    /// The file state for a code, `None` for codes this client does not know.
    #[must_use]
    pub fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            0 => Self::NormalOriginal,
            1 => Self::CorruptedInvalidCrc,
            2 => Self::SelfEdited,
            10 => Self::SelfRipped,
            11 => Self::OnDvd,
            12 => Self::OnVhs,
            13 => Self::OnTv,
            14 => Self::InTheaters,
            15 => Self::Streamed,
            100 => Self::Other,
            _ => return None,
        })
    }
}

/// One mylist entry. Dates are unix timestamps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MylistEntry {
    /// Mylist entry id.
    pub lid: i64,
    /// File id.
    pub fid: i64,
    /// Episode id.
    pub eid: i64,
    /// Anime id.
    pub aid: i64,
    /// Group id.
    pub gid: i64,
    /// Date the file was added to the mylist.
    pub date: i64,
    /// Storage state, `None` for a code this client does not know or an empty value.
    pub state: Option<MylistState>,
    /// Date the file was viewed, 0 when not viewed.
    pub viewdate: i64,
    /// Storage label.
    pub storage: String,
    /// Source, such as `ed2k`.
    pub source: String,
    /// Remarks, with line breaks as `\n`.
    pub other: String,
    /// File state, `None` for a code this client does not know or an empty value.
    pub filestate: Option<MylistFileState>,
}

/// A group of a 312 summary, with the episodes it has for the anime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MylistGroup {
    /// The short name of the group.
    pub group: String,
    /// The episodes of the group in the mylist, as text such as `1-12,14-16,T1`.
    pub episodes: String,
}

/// The summary of several entries, the 312 MULTIPLE MYLIST ENTRIES reply: the
/// anime title, the number of episodes, the episodes by state, the watched
/// episodes, then a short name and episode list per group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MylistSummary {
    /// The anime title.
    pub anime_title: String,
    /// The number of episodes.
    pub episodes: i64,
    /// Episodes whose state is unknown, as text.
    pub eps_unknown: String,
    /// Episodes on hdd, as text.
    pub eps_hdd: String,
    /// Episodes on cd, as text.
    pub eps_cd: String,
    /// Deleted episodes, as text.
    pub eps_deleted: String,
    /// Watched episodes, as text.
    pub eps_watched: String,
    /// The groups with entries.
    pub groups: Vec<MylistGroup>,
}

/// The answer to a MYLIST by anime: one entry (221), or the summary of several
/// (312).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MylistByAnime {
    /// 221 MYLIST: a single entry.
    Single(MylistEntry),
    /// 312 MULTIPLE MYLIST ENTRIES: a summary of several entries.
    Multiple(MylistSummary),
}
