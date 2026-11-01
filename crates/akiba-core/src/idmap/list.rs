//! `list.map.toml`: the `AniDB` to `AniList` mapping from Fribb's anime-list.

use serde_json::Value;

use crate::anime::{AnimeId, TitleVariant};
use crate::cache::paths::list_file;
use crate::cache::records::{ListEntry, ListFile};
use crate::cache::{CacheFile, KeyedMap};
use crate::config::Config;
use crate::http::{HttpClient, HttpError};

use super::{fill, mapped, IdMapProvider, Result};

/// Fribb's `anime-list-mini.json`. The mini variant lists only the ids.
pub const LIST_URL: &str =
    "https://raw.githubusercontent.com/Fribb/anime-lists/refs/heads/master/anime-list-mini.json";

/// The anime-list mapping. A cache file younger than `cache.mapping_age` days
/// is used as it is. Otherwise the list is downloaded and the cache rewritten.
#[derive(Debug, Clone)]
pub struct ListMap {
    file: CacheFile<ListFile>,
    map: ListFile,
}

impl ListMap {
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self {
            file: CacheFile::new(
                list_file(&config.cache.path),
                Some(config.cache.mapping_age),
            ),
            map: KeyedMap::default(),
        }
    }

    /// Loads a fresh cache, or downloads the list through `http` (use a client
    /// with a 30 second timeout).
    ///
    /// A fresh but unreadable cache is an error and nothing is downloaded. A
    /// failed download leaves the mapping empty. A failed write keeps the
    /// downloaded mapping in memory and returns the error.
    pub fn refresh(&mut self, http: &dyn HttpClient) -> Result<()> {
        if let Some(cached) = self.file.load()? {
            self.map = cached;
            return Ok(());
        }

        self.map = KeyedMap::default();
        let list = http.get_json(LIST_URL, &[])?;
        let entries = list
            .as_array()
            .ok_or_else(|| HttpError::new(LIST_URL, "expected a JSON array"))?;
        self.map = parse_list(entries);
        self.file.store(&self.map)?;
        Ok(())
    }

    /// The `AniList` id mapped to `aid`, if any.
    #[must_use]
    pub fn anilist(&self, aid: i64) -> Option<i64> {
        self.map.0.get(&aid).and_then(|entry| entry.anilist)
    }
}

/// Keeps the `AniList` id of each entry that has both ids. A later entry with the
/// same `AniDB` id wins, and an entry with a zero or missing `AniList` id changes
/// nothing. Other fields are ignored.
fn parse_list(entries: &[Value]) -> ListFile {
    let mut map = KeyedMap::default();
    for entry in entries {
        let Some(aid) = entry.get("anidb_id").and_then(Value::as_i64) else {
            continue;
        };
        let Some(anilist) = entry
            .get("anilist_id")
            .and_then(Value::as_i64)
            .filter(|&id| id != 0)
        else {
            continue;
        };
        let slot: &mut ListEntry = map.0.entry(aid).or_default();
        slot.anilist = Some(anilist);
    }
    map
}

impl IdMapProvider for ListMap {
    fn name(&self) -> &'static str {
        "list"
    }

    fn apply(&mut self, id: &mut AnimeId, _titles: &[TitleVariant], overwrite: bool) -> Result<()> {
        fill(&mut id.anilist, mapped(self.anilist(id.anidb)), overwrite);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
