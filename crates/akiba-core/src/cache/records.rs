//! Record types of the cache files that are not per-episode metadata. Each
//! alias names the type a cache file deserialises into.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::KeyedMap;
use crate::anime::TitleVariant;

/// One entry of the `<hostid>.hashes.toml` hash cache, keyed by absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashRecord {
    pub hash: String,
    pub size: u64,
    pub link: String,
    /// Epoch milliseconds, so cache files stay readable by akiba 2.x.
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}

/// `<hostid>.hashes.toml`, keyed by absolute episode path.
pub type HashesFile = BTreeMap<String, HashRecord>;

/// `titles.map.toml`: `AniDB` id to its official and main titles.
pub type TitlesFile = KeyedMap<Vec<TitleVariant>>;

/// One `AniDB` id in `list.map.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anilist: Option<i64>,
}

/// `list.map.toml`, keyed by `AniDB` id.
pub type ListFile = KeyedMap<ListEntry>;

/// One `AniDB` id in `local.map.toml`. The `-1` blacklist marker is stored here,
/// so the ids are signed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anilist: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmdb_season: Option<u32>,
}

/// `local.map.toml`, keyed by `AniDB` id.
pub type LocalFile = KeyedMap<LocalEntry>;
