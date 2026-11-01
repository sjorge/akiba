//! The `AniDB` side of the renamer: the [`AnidbFiles`] trait, its UDP
//! implementation and the mapping of FILE replies into the format record.

use anidb_udp::mask::{AnimeField, AnimeFields, FileField, FileFields};
use anidb_udp::mylist::MylistEntry;
use anidb_udp::{Client, ClientConfig, MylistAdd, ResponseCode};

use super::{AnidbError, RenamerError, Result};
use crate::config::Config;
use crate::fileinfo::FileInfo;
use crate::host_id;
use crate::logger::{self, Level};
use crate::num::parse_i64;

/// The `AniDB` calls the renamer needs.
pub trait AnidbFiles {
    /// FILE by hash, mapped into the format record. A failure with code 320
    /// means the hash is unknown to `AniDB`.
    fn file_by_hash(&mut self, ed2k: &str, size: u64) -> std::result::Result<FileInfo, AnidbError>;
    /// MYLIST by hash. A failure with code 321 means there is no entry.
    fn mylist_by_hash(
        &mut self,
        ed2k: &str,
        size: u64,
    ) -> std::result::Result<MylistEntry, AnidbError>;
    /// MYLISTADD by hash.
    fn mylist_add_by_hash(
        &mut self,
        ed2k: &str,
        size: u64,
        options: MylistAdd,
    ) -> std::result::Result<ResponseCode, AnidbError>;
    /// Close the session, if there is one.
    fn disconnect(&mut self);
}

/// The real [`AnidbFiles`]: an `anidb_udp::Client` that logs in on first use.
pub struct UdpFiles {
    client: Client,
    username: String,
    password: String,
    api_key: Option<String>,
}

impl UdpFiles {
    /// Reads the client settings from the config. Nothing is sent yet.
    ///
    /// # Errors
    ///
    /// [`RenamerError::Config`] when name, username or password is missing.
    pub fn new(config: &Config) -> Result<Self> {
        let udp = &config.anidb.udp_client;
        let (Some(name), Some(username), Some(password)) =
            (&udp.name, &udp.username, &udp.password)
        else {
            return Err(RenamerError::Config);
        };
        let mut client_config = ClientConfig::new(name.clone(), udp.version.unwrap_or(1));
        // one fixed source port per host, AniDB bans IPs that use many
        client_config.local_port = host_id::hostname()
            .ok()
            .map(|host| host_id::local_udp_port(&host));
        let mut client = Client::new(client_config);
        client.set_logger(|message| logger::log(message, Level::Warn));
        Ok(Self {
            client,
            username: username.clone(),
            password: password.clone(),
            api_key: udp.api_key.clone(),
        })
    }

    fn connect(&mut self) -> std::result::Result<(), AnidbError> {
        if self.client.is_logged_in() {
            return Ok(());
        }
        let result = self
            .client
            .connect(&self.username, &self.password, self.api_key.as_deref());
        if result.is_err() {
            self.client.disconnect();
        }
        result
    }
}

impl AnidbFiles for UdpFiles {
    fn file_by_hash(&mut self, ed2k: &str, size: u64) -> std::result::Result<FileInfo, AnidbError> {
        self.connect()?;
        let reply = self
            .client
            .file_by_hash(size, ed2k, &file_fields(), &anime_fields())?;
        Ok(file_info_from_reply(&reply))
    }

    fn mylist_by_hash(
        &mut self,
        ed2k: &str,
        size: u64,
    ) -> std::result::Result<MylistEntry, AnidbError> {
        self.connect()?;
        self.client.mylist_by_hash(size, ed2k)
    }

    fn mylist_add_by_hash(
        &mut self,
        ed2k: &str,
        size: u64,
        options: MylistAdd,
    ) -> std::result::Result<ResponseCode, AnidbError> {
        self.connect()?;
        self.client.mylist_add_by_hash(size, ed2k, options)
    }

    fn disconnect(&mut self) {
        if self.client.is_logged_in() {
            self.client.disconnect();
        }
    }
}

/// The FILE fields the renamer asks for (fmask `79f8fff100`).
#[must_use]
pub fn file_fields() -> FileFields {
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

/// The anime fields the renamer asks for (amask `f2fcf0c0`).
#[must_use]
pub fn anime_fields() -> AnimeFields {
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

const STATUS_ISV2: i64 = 0x04;
const STATUS_ISV3: i64 = 0x08;
const STATUS_ISV4: i64 = 0x10;
const STATUS_ISV5: i64 = 0x20;
const STATUS_UNC: i64 = 0x40;
const STATUS_CEN: i64 = 0x80;

/// Maps a FILE reply into the format record, the way `getAniDBFileByHash` does.
#[must_use]
pub fn file_info_from_reply(reply: &anidb_udp::file::FileReply) -> FileInfo {
    let int = |field: FileField| reply.file_int(field).unwrap_or(0);
    let text = |field: FileField| reply.file_str(field).unwrap_or_default().to_owned();
    // The text items of a list field, or its numbers printed in decimal.
    let joined = |field: FileField| {
        if let Some(items) = reply.file_strs(field) {
            items.join("'")
        } else {
            let numbers = reply.file_ints(field).unwrap_or_default();
            numbers
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join("'")
        }
    };
    let aint = |field: AnimeField| reply.anime_int(field).unwrap_or(0);
    let atext = |field: AnimeField| reply.anime_str(field).unwrap_or_default().to_owned();
    let alist = |field: AnimeField| reply.anime_strs(field).unwrap_or_default().to_vec();

    let status = int(FileField::State);
    let mut version = "";
    for (bit, name) in [
        (STATUS_ISV2, "v2"),
        (STATUS_ISV3, "v3"),
        (STATUS_ISV4, "v4"),
        (STATUS_ISV5, "v5"),
    ] {
        if status & bit > 0 {
            version = name;
        }
    }
    let mut censored = "";
    if status & STATUS_CEN > 0 {
        censored = "cen";
    }
    if status & STATUS_UNC > 0 {
        censored = "unc";
    }
    let video_bitrate = int(FileField::VideoBitrate);

    FileInfo {
        fid: reply.fid,
        aid: int(FileField::Aid),
        eid: int(FileField::Eid),
        gid: int(FileField::Gid),
        lid: int(FileField::MylistId),
        status,
        size: u64::try_from(int(FileField::Size)).unwrap_or(0),
        ed2k: text(FileField::Ed2k),
        md5: text(FileField::Md5),
        sha1: text(FileField::Sha1),
        crc32: text(FileField::Crc32),
        lang_dub: joined(FileField::DubLanguage),
        lang_sub: joined(FileField::SubLanguage),
        quality: text(FileField::Quality),
        source: text(FileField::Source),
        audio_codec: joined(FileField::AudioCodecs),
        audio_bitrate: joined(FileField::AudioBitrates),
        video_codec: text(FileField::VideoCodec),
        video_bitrate: if video_bitrate == 0 {
            String::new()
        } else {
            video_bitrate.to_string()
        },
        resolution: text(FileField::VideoResolution),
        filetype: text(FileField::FileExtension),
        length: int(FileField::LengthSeconds),
        description: text(FileField::Description),
        group: atext(AnimeField::GroupName),
        group_short: atext(AnimeField::GroupShortName),
        episode: atext(AnimeField::Epno),
        episode_name: atext(AnimeField::EpName),
        episode_name_romaji: atext(AnimeField::EpRomanjiName),
        episode_name_kanji: atext(AnimeField::EpKanjiName),
        episode_total: aint(AnimeField::AnimeTotalEpisodes),
        episode_last: aint(AnimeField::HighestEpisode),
        // NOTE: can be YYYY-YYYY; a part that is not a whole number is dropped
        anime_year: atext(AnimeField::Year)
            .split('-')
            .filter_map(parse_i64)
            .collect(),
        anime_type: atext(AnimeField::Type),
        anime_name_romaji: atext(AnimeField::RomanjiName),
        anime_name_kanji: atext(AnimeField::KanjiName),
        anime_name_english: atext(AnimeField::EnglishName),
        anime_name_other: alist(AnimeField::OtherNameList),
        anime_name_short: alist(AnimeField::ShortNameList),
        anime_synonyms: alist(AnimeField::SynonymList),
        anime_category: alist(AnimeField::CategoryList),
        version: version.to_owned(),
        censored: censored.to_owned(),
        original_name: text(FileField::AnidbFilename),
    }
}
