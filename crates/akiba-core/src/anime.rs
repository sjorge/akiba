//! Shared anime types: ids across providers, title variants and episode extensions.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Identifiers of one anime across the providers. `anidb` is always known.
/// External ids are `i64`, since the legacy caches store `-1` as a blacklist
/// marker and TOML integers are `i64`. Season numbers are not ids, so they stay
/// `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimeId {
    pub anidb: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anilist: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvdb: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvdb_season: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb_season: Option<u32>,
}

/// One title of an anime, as listed in the `AniDB` titles dump and the cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleVariant {
    pub title: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    pub language: String,
}

/// File extensions treated as episodes, without the dot.
pub const EPISODE_EXTENSIONS: &[&str] = &["mkv", "mp4", "ogm", "avi"];

/// Whether `path` has one of the [`EPISODE_EXTENSIONS`]. Matching is
/// case-sensitive: `Show.MKV` is not an episode.
#[must_use]
pub fn is_episode_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| EPISODE_EXTENSIONS.contains(&ext))
}
