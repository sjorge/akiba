//! External id mapping. Each provider fills in the missing ids of an
//! [`AnimeId`]: the local mapping file, the anime-list mapping, and the `AniList`
//! and `TMDB` searches.
//!
//! `AniDB` is the main data source. `AniList` and `TMDB` only supply ids for
//! external software, so their providers stay small.

pub mod anilist;
pub mod list;
pub mod local;
pub mod tmdb;

pub use anilist::AniList;
pub use list::ListMap;
pub use local::LocalMap;
pub use tmdb::Tmdb;

use crate::anime::{AnimeId, TitleVariant};
use crate::cache::CacheError;
use crate::http::HttpError;

/// Errors from the id providers. A failed lookup is an error, not a silent
/// miss, so the caller can log it.
#[derive(Debug, thiserror::Error)]
pub enum IdMapError {
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error("cannot read the {service} reply: {source}")]
    Reply {
        service: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("No Anilist token configured.")]
    MissingAnilistToken,
    #[error("No Tmdb API key configured.")]
    MissingTmdbKey,
}

pub type Result<T> = std::result::Result<T, IdMapError>;

/// One source of external ids.
pub trait IdMapProvider {
    /// Name used in log lines.
    fn name(&self) -> &'static str;

    /// Fills in the external ids of `id` that this provider knows. A value that
    /// is already set stays, unless `overwrite` is true. `titles` are all the
    /// known titles of the anime, and a provider picks the ones it searches for.
    fn apply(&mut self, id: &mut AnimeId, titles: &[TitleVariant], overwrite: bool) -> Result<()>;
}

/// A mapped id counts only when it is non-zero. The `-1` blacklist marker is kept, so it still counts.
fn mapped(value: Option<i64>) -> Option<i64> {
    value.filter(|&v| v != 0)
}

/// Sets `slot` from a mapping when it is empty or `overwrite` is set.
fn fill(slot: &mut Option<i64>, mapped_value: Option<i64>, overwrite: bool) {
    if let Some(value) = mapped_value {
        if slot.is_none() || overwrite {
            *slot = Some(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_not_a_mapping_but_the_blacklist_marker_is() {
        assert_eq!(mapped(Some(0)), None);
        assert_eq!(mapped(Some(-1)), Some(-1));
        assert_eq!(mapped(None), None);
    }

    #[test]
    fn fill_respects_overwrite() {
        let mut slot = Some(99);
        fill(&mut slot, Some(10), false);
        assert_eq!(slot, Some(99));
        fill(&mut slot, Some(10), true);
        assert_eq!(slot, Some(10));

        let mut empty = None;
        fill(&mut empty, None, true);
        assert_eq!(empty, None);
    }
}
