//! `local.map.toml`: ids that the user maps by hand, with `-1` as the blacklist
//! marker.

use crate::anime::{AnimeId, TitleVariant};
use crate::cache::paths::local_file;
use crate::cache::records::{LocalEntry, LocalFile};
use crate::cache::{CacheFile, KeyedMap};
use crate::config::Config;

use super::{fill, mapped, IdMapProvider, Result};

/// The local mapping, loaded from `local.map.toml` in the cache directory.
#[derive(Debug, Clone)]
pub struct LocalMap {
    file: CacheFile<LocalFile>,
    map: LocalFile,
}

impl LocalMap {
    /// The mapping lives in `config.cache.path` and never expires.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self {
            file: CacheFile::new(local_file(&config.cache.path), None),
            map: KeyedMap::default(),
        }
    }

    /// Loads the mapping file. A missing file gives `Ok(false)` with an empty
    /// mapping. An unreadable file is an error, so the caller can tell it apart
    /// from a missing one.
    pub fn refresh(&mut self) -> Result<bool> {
        let loaded = self.file.load()?;
        let present = loaded.is_some();
        self.map = loaded.unwrap_or_default();
        Ok(present)
    }

    /// The mapping entry of `aid`, if there is one.
    #[must_use]
    pub fn entry(&self, aid: i64) -> Option<&LocalEntry> {
        self.map.0.get(&aid)
    }

    /// Maps `aid` to an `AniList` id.
    pub fn set_anilist(&mut self, aid: i64, anilist: i64) {
        self.map.0.entry(aid).or_default().anilist = Some(anilist);
    }

    /// Removes the `AniList` mapping of `aid`. Returns whether there was one.
    pub fn unset_anilist(&mut self, aid: i64) -> bool {
        let removed = self
            .map
            .0
            .get_mut(&aid)
            .and_then(|entry| entry.anilist.take())
            .is_some();
        self.prune(aid);
        removed
    }

    /// Maps `aid` to a TMDB series id. The season is always 1.
    pub fn set_tmdb(&mut self, aid: i64, tmdb: i64) {
        let entry = self.map.0.entry(aid).or_default();
        entry.tmdb = Some(tmdb);
        entry.tmdb_season = Some(1);
    }

    /// Removes the TMDB mapping of `aid`, and its season. Returns whether there
    /// was one.
    pub fn unset_tmdb(&mut self, aid: i64) -> bool {
        let mut removed = false;
        if let Some(entry) = self.map.0.get_mut(&aid) {
            if entry.tmdb.take().is_some() {
                entry.tmdb_season = None;
                removed = true;
            }
        }
        self.prune(aid);
        removed
    }

    /// Writes the mapping atomically with mode 0660, creating the cache
    /// directory when it is missing.
    pub fn save(&self) -> Result<()> {
        self.file.store(&self.map)?;
        Ok(())
    }

    /// Drops the entry of `aid` once it maps nothing, as the `idmap` command did.
    fn prune(&mut self, aid: i64) {
        if self
            .map
            .0
            .get(&aid)
            .is_some_and(|entry| entry.anilist.is_none() && entry.tmdb.is_none())
        {
            self.map.0.remove(&aid);
        }
    }
}

impl IdMapProvider for LocalMap {
    fn name(&self) -> &'static str {
        "local"
    }

    fn apply(&mut self, id: &mut AnimeId, _titles: &[TitleVariant], overwrite: bool) -> Result<()> {
        let Some(entry) = self.map.0.get(&id.anidb) else {
            return Ok(());
        };
        fill(&mut id.anilist, mapped(entry.anilist), overwrite);
        if let Some(tmdb) = mapped(entry.tmdb) {
            if id.tmdb.is_none() || overwrite {
                id.tmdb = Some(tmdb);
                // Copied as is, so an overwrite can clear the season too.
                id.tmdb_season = entry.tmdb_season;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
