//! The typed parameters of the client commands, and the mylist export template
//! names. The methods that take them are in [`crate::commands`].

use crate::mylist::MylistState;

/// An id or a name: many commands take either (`aid=` or `aname=`, `gid=` or
/// `gname=`, `uid=` or `user=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdOrName<'a> {
    /// The numeric id.
    Id(i64),
    /// The name, which the server matches exactly.
    Name(&'a str),
}

impl From<i64> for IdOrName<'_> {
    fn from(id: i64) -> Self {
        Self::Id(id)
    }
}

impl<'a> From<&'a str> for IdOrName<'a> {
    fn from(name: &'a str) -> Self {
        Self::Name(name)
    }
}

impl IdOrName<'_> {
    /// An id of 0 and an empty name count as not given.
    pub(crate) const fn is_empty(self) -> bool {
        match self {
            Self::Id(id) => id == 0,
            Self::Name(name) => name.is_empty(),
        }
    }
}

/// The character sets the server can switch to, named as the Java charsets the
/// `AniDB` spec links to. AUTH with `enc=` switches right away, and silently
/// ignores a set it does not support. ENCODING answers 519 for such a set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Encoding {
    /// `ISO8859_1`
    Iso8859_1,
    /// `ISO8859_2`
    Iso8859_2,
    /// `ISO8859_4`
    Iso8859_4,
    /// `ISO8859_5`
    Iso8859_5,
    /// `ISO8859_7`
    Iso8859_7,
    /// `ISO8859_9`
    Iso8859_9,
    /// `ISO8859_13`
    Iso8859_13,
    /// `ISO8859_15`
    Iso8859_15,
    /// `KOI8_R`
    Koi8R,
    /// `ASCII`, the default of the server.
    Ascii,
    /// `UTF8`, what this client asks for.
    #[default]
    Utf8,
    /// `UTF-16`
    Utf16,
    /// `UnicodeBigUnmarked`
    UnicodeBigUnmarked,
    /// `UnicodeLittleUnmarked`
    UnicodeLittleUnmarked,
    /// `Cp1250`
    Cp1250,
    /// `Cp1251`
    Cp1251,
    /// `Cp1252`
    Cp1252,
    /// `Cp1253`
    Cp1253,
    /// `Cp1254`
    Cp1254,
    /// `Cp1257`
    Cp1257,
    /// `UnicodeBig`
    UnicodeBig,
    /// `UnicodeLittle`
    UnicodeLittle,
}

impl Encoding {
    /// The name sent as `name=`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Iso8859_1 => "ISO8859_1",
            Self::Iso8859_2 => "ISO8859_2",
            Self::Iso8859_4 => "ISO8859_4",
            Self::Iso8859_5 => "ISO8859_5",
            Self::Iso8859_7 => "ISO8859_7",
            Self::Iso8859_9 => "ISO8859_9",
            Self::Iso8859_13 => "ISO8859_13",
            Self::Iso8859_15 => "ISO8859_15",
            Self::Koi8R => "KOI8_R",
            Self::Ascii => "ASCII",
            Self::Utf8 => "UTF8",
            Self::Utf16 => "UTF-16",
            Self::UnicodeBigUnmarked => "UnicodeBigUnmarked",
            Self::UnicodeLittleUnmarked => "UnicodeLittleUnmarked",
            Self::Cp1250 => "Cp1250",
            Self::Cp1251 => "Cp1251",
            Self::Cp1252 => "Cp1252",
            Self::Cp1253 => "Cp1253",
            Self::Cp1254 => "Cp1254",
            Self::Cp1257 => "Cp1257",
            Self::UnicodeBig => "UnicodeBig",
            Self::UnicodeLittle => "UnicodeLittle",
        }
    }
}

/// How UPDATED looks back. The server takes one of the two, never both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatedSince {
    /// `age=`: that many days.
    Age(u32),
    /// `time=`: unix time.
    Time(i64),
}

/// The file a MYLISTADD or MYLISTDEL is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MylistFile<'a> {
    /// `fid=`: the file id.
    Fid(i64),
    /// `size=&ed2k=`: the file by hash.
    Hash {
        /// The ed2k hash.
        ed2k: &'a str,
        /// The size in bytes.
        size: u64,
    },
    /// `lid=`: the mylist entry id. A MYLISTADD by `lid` only edits. The server
    /// does less work when the lid is known, so use it then.
    Lid(i64),
}

/// The options of MYLISTADD.
/// Fields left `None` are not sent, so an edit keeps their value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MylistAddOptions {
    /// `edit=`: edit an entry that exists, instead of failing with 310. A
    /// MYLISTADD by `lid` implies `Some(true)` and fails with `Some(false)`.
    pub edit: Option<bool>,
    /// The storage state.
    pub state: Option<MylistState>,
    /// Whether the file was watched.
    pub viewed: Option<bool>,
    /// When it was watched, unix time. Without it the server uses the current
    /// time. The server ignores it for `viewed` false.
    pub viewdate: Option<i64>,
    /// The source, such as `ed2k`, `dc` or `irc`.
    pub source: Option<String>,
    /// The label of the storage.
    pub storage: Option<String>,
    /// Remarks. The only field that may hold newlines.
    pub other: Option<String>,
}

/// The outcome of [`Client::mylist_add_by_file`](crate::Client::mylist_add_by_file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MylistAdded {
    /// 210: a new entry with this `lid`.
    Added {
        /// The id of the new mylist entry.
        lid: i64,
    },
    /// 311: this many entries were edited. A 311 without the count (the spec
    /// allows both forms) is reported as one entry edited.
    Edited {
        /// The number of entries edited.
        edited: i64,
    },
}

/// The anime (and group, and episode) of a MYLISTADD by anime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MylistAnimeSpec<'a> {
    /// The anime.
    pub anime: IdOrName<'a>,
    /// The group. Without one `generic=1` is sent, which adds the generic file
    /// instead of the episode of every group. The spec makes the group optional
    /// only for edits; sending `generic=1` is a deliberate deviation, see the
    /// request builder.
    pub group: Option<IdOrName<'a>>,
    /// 0 for all episodes, a negative number for up to that episode (-12 is up
    /// to 12).
    pub epno: i64,
}

/// The outcome of [`Client::mylist_add_by_anime`](crate::Client::mylist_add_by_anime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MylistAddedByAnime {
    /// 210: this many entries were added.
    Added(i64),
    /// 311: this many entries were edited. A 311 without the count (the spec
    /// allows both forms) is reported as one entry edited.
    Edited(i64),
}

/// What a VOTE asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteChoice {
    /// Retrieve the current vote (`value=0`).
    Get,
    /// Revoke the vote (`value=-1`).
    Revoke,
    /// Vote: 100 to 1000, anything else is illegal input.
    Value(u16),
}

/// The list RANDOMANIME draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomType {
    /// 0: the whole database.
    Db,
    /// 1: watched anime.
    Watched,
    /// 2: unwatched anime.
    Unwatched,
    /// 3: the whole mylist.
    Mylist,
}

/// The mylist export templates known when this was written. The spec only says
/// that a template name must match one on the `MyList` Export page of the
/// website, so the server has the last word. It answers 317 for an unknown
/// template.
pub const EXPORT_TEMPLATES: &[&str] = &[
    "csv-adborg",
    "csv-files",
    "csv-minimal",
    "html-anidbstyle",
    "html-cleancut",
    "html-cloud",
    "html-dump",
    "html-fixme",
    "html-fstyle",
    "html-goldsilver",
    "html-graywhite",
    "html-green",
    "html-knch",
    "html-lightblue",
    "html-operatic",
    "html-orangewhite",
    "html-silver",
    "json-adborg",
    "json-large",
    "plot-time-wasted",
    "sfv-simple",
    "txt-ed2k",
    "txt-udp-mylist",
    "xml",
    "xml-acm",
    "xml-anivote",
    "xml-cdb",
    "xml-dark",
    "xml-files",
    "xml-large",
    "xml-mini",
    "xml-mystatus",
    "xml-naive",
    "xml-nx",
    "xml-plain",
    "xml-plain-anisubtwo",
    "xml-plain-cs",
    "xml-plain-full",
    "xml-plain-new",
    "xml-xslt",
];
