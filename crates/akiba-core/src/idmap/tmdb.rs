//! TMDB series lookup by title.
//!
//! Only `search/tv` is used, with the same query string that `moviedb-promise`
//! sends. Results without the Animation genre (16) are ignored.

use serde::Deserialize;

use crate::anime::{AnimeId, TitleVariant};
use crate::http::{percent_encode, with_query, HttpClient, HttpError};
use crate::idmap::{IdMapError, IdMapProvider, Result};

const SEARCH_URL: &str = "https://api.themoviedb.org/3/search/tv";

/// TMDB genre id of Animation.
const ANIMATION_GENRE: i64 = 16;

/// One TV series from `search/tv`. Only the fields the match uses are read.
#[derive(Debug, Deserialize)]
struct Series {
    id: i64,
    name: Option<String>,
    original_name: Option<String>,
    original_language: Option<String>,
    #[serde(default)]
    genre_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct SearchReply {
    #[serde(default)]
    results: Vec<Series>,
}

/// TMDB client for the series lookup. It holds the API key and the transport.
pub struct Tmdb<'a> {
    http: &'a dyn HttpClient,
    api_key: String,
}

impl<'a> Tmdb<'a> {
    /// Fails when no API key is configured. An empty key counts as none, .
    pub fn new(http: &'a dyn HttpClient, api_key: Option<&str>) -> Result<Self> {
        let api_key = api_key
            .filter(|key| !key.is_empty())
            .ok_or(IdMapError::MissingTmdbKey)?;
        Ok(Self {
            http,
            api_key: api_key.to_owned(),
        })
    }

    fn search_tv(&self, query: &str) -> Result<Vec<Series>> {
        let url = with_query(
            SEARCH_URL,
            &[
                ("api_key", self.api_key.as_str()),
                ("query", query),
                ("include_adult", "true"),
            ],
        );
        let reply = self
            .http
            .get_json(&url, &[])
            .map_err(|err| self.redact(err))?;
        let parsed: SearchReply =
            serde_json::from_value(reply).map_err(|source| IdMapError::Reply {
                service: "TMDB",
                source,
            })?;
        Ok(parsed.results)
    }

    /// Keeps the API key out of error messages. The key is in the query string.
    fn redact(&self, mut err: HttpError) -> IdMapError {
        for secret in [
            self.api_key.as_str(),
            percent_encode(&self.api_key).as_str(),
        ] {
            if !secret.is_empty() {
                err.url = err.url.replace(secret, "***");
                err.message = err.message.replace(secret, "***");
            }
        }
        IdMapError::Http(err)
    }

    /// The series id that matches one of `titles`. The first exact match wins.
    /// Failing that, the first Japanese match after normalisation wins.
    fn lookup(&self, titles: &[TitleVariant]) -> Result<Option<i64>> {
        let mut exact: Option<i64> = None;
        let mut best: Option<i64> = None;

        for title in titles.iter().filter(|t| is_lookup_title(t)) {
            if exact.is_some() {
                break;
            }
            for series in self.search_tv(&title.title)? {
                if exact.is_some() {
                    break;
                }
                if !series.genre_ids.contains(&ANIMATION_GENRE) {
                    continue;
                }

                let japanese =
                    title.language == "ja" && series.original_language.as_deref() == Some("ja");
                let name = if japanese {
                    series.original_name.as_deref()
                } else {
                    series.name.as_deref()
                };
                let Some(name) = name else {
                    continue;
                };
                let name = name.to_lowercase();

                if name == title.title.to_lowercase() {
                    exact = Some(series.id);
                } else if japanese && best.is_none() && name == normalise_japanese(&title.title) {
                    best = Some(series.id);
                }
            }
        }

        Ok(exact.or(best))
    }
}

impl IdMapProvider for Tmdb<'_> {
    fn name(&self) -> &'static str {
        "tmdb"
    }

    fn apply(&mut self, id: &mut AnimeId, titles: &[TitleVariant], overwrite: bool) -> Result<()> {
        if id.tmdb.is_some() && !overwrite {
            return Ok(());
        }
        if let Some(found) = self.lookup(titles)? {
            id.tmdb = Some(found);
            id.tmdb_season = Some(1);
        }
        Ok(())
    }
}

/// The official Japanese or English title and the main romaji title.
fn is_lookup_title(title: &TitleVariant) -> bool {
    matches!(
        (title.kind.as_deref(), title.language.as_str()),
        (Some("official"), "ja" | "en") | (Some("main"), "x-jat")
    )
}

/// The lowercased title with the punctuation that TMDB writes in full-width or
/// Japanese form: `&` is `and`, `[]` are `【】`, a final `.` is `。` and `,` is `、`.
fn normalise_japanese(title: &str) -> String {
    let lowered = title
        .to_lowercase()
        .replace('&', "and")
        .replace('[', "【")
        .replace(']', "】");
    let ended = match lowered.strip_suffix('.') {
        Some(rest) => format!("{rest}。"),
        None => lowered,
    };
    ended.replace(',', "、")
}

#[cfg(test)]
mod tests;
