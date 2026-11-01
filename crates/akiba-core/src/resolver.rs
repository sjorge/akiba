//! Titles cache, anime id lookup from a name, and episode discovery in a show
//! directory.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anidb_http::{
    fetch_titles_dump_with, parse_titles, TlsOptions, Transport, UreqTransport, TITLES_DUMP_URL,
};
use regex::Regex;

use crate::anime::{is_episode_file, AnimeId, TitleVariant};
use crate::cache::paths::titles_file;
use crate::cache::records::TitlesFile;
use crate::cache::{CacheError, CacheFile, KeyedMap};
use crate::config::Config;
use crate::http::DEFAULT_TIMEOUT;
use crate::text;

/// Largest edit distance that still counts as a fuzzy title match.
const FUZZY_THRESHOLD: usize = 3;

/// One episode in a file name stem: ` - <episode> - <title>`, where the episode
/// is a number with an optional special prefix (`S`pecial, `C`redits,
/// `T`railer, `P`arody, `O`ther, `E`pisode). The first ` - <episode> - ` wins and
/// the title is the rest of the stem, so it may contain ` - ` itself:
///
/// - `Show - 01 - Title (0ba1abec)`: episode `01`, title `Title (0ba1abec)`
/// - `Show - S1 - Special`: episode `S1`, title `Special`
/// - `Show - 03 - Part 1 - The Return`: episode `03`, title `Part 1 - The Return`
///
/// The `expect` cannot fail: the pattern is a fixed, valid literal.
static SINGLE_EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s-\s(?<episode>[SCTPOE]?[0-9]+)\s-\s(?<title>.+)$").expect("valid regex")
});

/// An episode range in a file name stem: ` - <first>-<last> - <title>`, each
/// end with an optional special prefix (no `E` here):
///
/// - `Show - 01-02 - Double`: `01` to `02`, title `Double`
/// - `Show - S1-S3 - Specials`: `S1` to `S3`, title `Specials`
///
/// The `expect` cannot fail: the pattern is a fixed, valid literal.
static MULTI_EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s-\s(?<first>[SCTPO]?[0-9]+)-(?<last>[SCTPO]?[0-9]+)\s-\s(?<title>.+)$")
        .expect("valid regex")
});

/// Errors from the titles cache, the titles download and episode discovery.
#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error(transparent)]
    Titles(#[from] anidb_http::Error),
    #[error(transparent)]
    Tls(#[from] anidb_http::TlsError),
    #[error("{}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

pub type Result<T> = std::result::Result<T, ResolverError>;

/// One episode file found by [`Resolver::episodes`]. Episode numbers stay
/// strings, because specials such as `S1` and `C2` are not numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeFile {
    pub path: PathBuf,
    pub episode_start: String,
    pub episode_end: String,
    pub title: String,
}

/// Title lookup and matching. Build it from a [`Config`], then [`refresh`]
/// before the first lookup.
///
/// [`refresh`]: Resolver::refresh
#[derive(Debug, Clone)]
pub struct Resolver {
    file: CacheFile<TitlesFile>,
    titles: BTreeMap<i64, Vec<TitleVariant>>,
    tls: TlsOptions,
}

impl Resolver {
    /// The titles cache lives in `config.cache.path` and expires after
    /// `config.cache.title_age` days.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self {
            file: CacheFile::new(
                titles_file(&config.cache.path),
                Some(config.cache.title_age),
            ),
            titles: BTreeMap::new(),
            tls: config.tls_options(),
        }
    }

    /// Loads the titles cache when it is fresh, otherwise downloads the titles
    /// dump and rewrites the cache. `force` skips the freshness check.
    pub fn refresh(&mut self, force: bool) -> Result<()> {
        let transport = UreqTransport::new(DEFAULT_TIMEOUT, &self.tls)?;
        self.refresh_with(&transport, force)
    }

    /// Same as [`Resolver::refresh`], with the download going through `transport`.
    pub fn refresh_with(&mut self, transport: &dyn Transport, force: bool) -> Result<()> {
        if !force {
            if let Some(KeyedMap(cached)) = self.file.load()? {
                self.titles = cached;
                return Ok(());
            }
        }

        let dump = fetch_titles_dump_with(transport, TITLES_DUMP_URL)?;
        let titles: BTreeMap<i64, Vec<TitleVariant>> = parse_titles(&dump)?
            .into_iter()
            .map(|(aid, variants)| {
                (
                    i64::from(aid),
                    variants.into_iter().map(from_dump).collect(),
                )
            })
            .collect();
        let file = KeyedMap(titles);
        self.file.store(&file)?;
        self.titles = file.0;
        Ok(())
    }

    /// The official and main titles of `id`. Empty when the id is unknown.
    #[must_use]
    pub fn titles(&self, id: &AnimeId) -> &[TitleVariant] {
        match self.titles.get(&id.anidb) {
            Some(variants) => variants,
            None => &[],
        }
    }

    /// The anime id for a file or directory name.
    ///
    /// An `[anidb-N]` tag wins. Otherwise an exact title match wins, and then
    /// the closest title within `FUZZY_THRESHOLD`. Ties go to the lowest aid,
    /// and an exact match goes to the last one in aid order.
    #[must_use]
    pub fn id(&self, title: &str) -> Option<AnimeId> {
        if let Some(aid) = tagged_aid(title) {
            return Some(anidb_only(aid));
        }

        let normalized = title.replace('\u{2044}', "/");
        let query = text::fold(&normalized);

        let mut exact = None;
        let mut best: Option<(i64, usize)> = None;
        for (&aid, variants) in &self.titles {
            for variant in variants {
                if variant.title == normalized {
                    exact = Some(aid);
                    continue;
                }
                let distance = text::levenshtein_folded(&query, &text::fold(&variant.title));
                if distance <= FUZZY_THRESHOLD && best.is_none_or(|(_, score)| score > distance) {
                    best = Some((aid, distance));
                }
            }
        }

        exact.or(best.map(|(aid, _)| aid)).map(anidb_only)
    }

    /// The episode files in `dir`, in natural order. A missing path or a file
    /// gives an empty list. Names that are not UTF-8 are skipped, because the
    /// episode patterns are text patterns.
    pub fn episodes(dir: &Path) -> Result<Vec<EpisodeFile>> {
        let metadata = match fs::metadata(dir) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(io_error(dir, source)),
        };
        if !metadata.is_dir() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in fs::read_dir(dir).map_err(|source| io_error(dir, source))? {
            let entry = entry.map_err(|source| io_error(dir, source))?;
            if let Ok(name) = entry.file_name().into_string() {
                names.push(name);
            }
        }
        names.sort_by(|a, b| text::natural_cmp(a, b));

        let mut episodes = Vec::new();
        for name in names {
            if !is_episode_file(Path::new(&name)) {
                continue;
            }
            let Some((title, numbers)) = parse_episode_name(&name) else {
                continue;
            };
            episodes.push(EpisodeFile {
                path: dir.join(&name),
                episode_start: numbers.first().cloned().unwrap_or_default(),
                episode_end: numbers.last().cloned().unwrap_or_default(),
                title,
            });
        }
        Ok(episodes)
    }
}

fn anidb_only(aid: i64) -> AnimeId {
    AnimeId {
        anidb: aid,
        anilist: None,
        tvdb: None,
        tvdb_season: None,
        tmdb: None,
        tmdb_season: None,
    }
}

fn from_dump(variant: anidb_http::TitleVariant) -> TitleVariant {
    TitleVariant {
        title: variant.title,
        kind: variant.kind,
        year: None,
        language: variant.language,
    }
}

fn io_error(path: &Path, source: io::Error) -> ResolverError {
    ResolverError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// The aid of the first `[anidb-N]` tag in a name, such as `Show [anidb-123]`.
/// A tag whose number does not fit an `i64` is no match.
fn tagged_aid(name: &str) -> Option<i64> {
    let digits = name.match_indices("[anidb-").find_map(|(start, tag)| {
        let (digits, _) = name[start + tag.len()..].split_once(']')?;
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(digits)
    })?;
    digits.parse().ok()
}

/// Splits an episode file name into its title and episode numbers. The
/// extension (already checked by the caller) is dropped first, then the single
/// episode pattern is tried before the range pattern.
fn parse_episode_name(name: &str) -> Option<(String, Vec<String>)> {
    let (stem, _extension) = name.rsplit_once('.')?;
    let (title, numbers) = if let Some(caps) = SINGLE_EPISODE.captures(stem) {
        (
            caps.name("title")?.as_str(),
            vec![episode_number(&caps["episode"])],
        )
    } else {
        let caps = MULTI_EPISODE.captures(stem)?;
        let numbers = vec![
            episode_number(&caps["first"]),
            episode_number(&caps["last"]),
        ];
        (caps.name("title")?.as_str(), numbers)
    };
    Some((strip_checksum(title).trim().to_owned(), numbers))
}

/// Removes a CRC32 checksum such as `(0ba1abec)` from an episode title: the
/// last `(` + eight ASCII letters or digits + `)` that does not start the title.
/// It is usually at the end (`Title (0ba1abec)`), but need not be
/// (`Title (0ba1abec) [v2]`). The first occurrence of that text is
/// removed, which only differs when the same checksum appears twice.
fn strip_checksum(title: &str) -> String {
    let is_checksum = |group: &str| {
        group.len() == 10
            && group.ends_with(')')
            && group.as_bytes()[1..9].iter().all(u8::is_ascii_alphanumeric)
    };
    let checksum = title
        .match_indices('(')
        .rev()
        .filter(|&(start, _)| start > 0)
        .find_map(|(start, _)| {
            title
                .get(start..start + 10)
                .filter(|group| is_checksum(group))
        });
    match checksum {
        Some(checksum) => title.replacen(checksum, "", 1),
        None => title.to_owned(),
    }
}

/// An episode number as written in a file name. Plain digits lose their leading
/// zeros (`01` is `1`); a value with a type prefix keeps its identity and is
/// kept exactly as written (`S01` stays `S01`, never the normal episode `1`).
fn episode_number(text: &str) -> String {
    if text.bytes().all(|b| b.is_ascii_digit()) {
        strip_leading_zeros(text)
    } else {
        text.to_owned()
    }
}

/// A run of ASCII digits with leading zeros removed (`000` is `0`). Working on
/// the digits avoids overflow for long runs.
fn strip_leading_zeros(digits: &str) -> String {
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests;
