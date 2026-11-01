//! Per-file metadata record, the `<ed2k>.fid.toml` cache entry. The formatter tokens read its fields.
//! The legacy typos `quaility` and `orginal_name` are kept as serialised names.

use serde::{Deserialize, Serialize};

/// Metadata of one episode file, as cached by the renamer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileInfo {
    pub fid: i64,
    pub aid: i64,
    pub eid: i64,
    pub gid: i64,
    pub lid: i64,
    pub status: i64,
    pub size: u64,
    pub ed2k: String,
    pub md5: String,
    pub sha1: String,
    pub crc32: String,
    pub lang_dub: String,
    pub lang_sub: String,
    #[serde(rename = "quaility")]
    pub quality: String,
    pub source: String,
    pub audio_codec: String,
    pub audio_bitrate: String,
    pub video_codec: String,
    pub video_bitrate: String,
    pub resolution: String,
    pub filetype: String,
    pub length: i64,
    pub description: String,
    pub group: String,
    pub group_short: String,
    pub episode: String,
    pub episode_name: String,
    pub episode_name_romaji: String,
    pub episode_name_kanji: String,
    pub episode_total: i64,
    pub episode_last: i64,
    pub anime_year: Vec<i64>,
    pub anime_type: String,
    pub anime_name_romaji: String,
    pub anime_name_kanji: String,
    pub anime_name_english: String,
    pub anime_name_other: Vec<String>,
    pub anime_name_short: Vec<String>,
    pub anime_synonyms: Vec<String>,
    pub anime_category: Vec<String>,
    pub version: String,
    pub censored: String,
    #[serde(rename = "orginal_name")]
    pub original_name: String,
}
