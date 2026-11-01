//! `AniList` lookup by title.
//!
//! One query searches anime by title and one reads an anime by id, each asking
//! for only the fields the match uses. A GraphQL `errors` reply
//! is no match. An HTTP failure is an error.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::anime::{AnimeId, TitleVariant};
use crate::http::HttpClient;
use crate::idmap::{IdMapError, IdMapProvider, Result};
use crate::text;

const ENDPOINT: &str = "https://graphql.anilist.co";

const SEARCH_QUERY: &str = "query ($page: Int, $perPage: Int, $search: String) { Page (page: $page, perPage: $perPage) { media (type: ANIME, search: $search) { id title { romaji native } } } }";

const MEDIA_QUERY: &str = "query ($id: Int) { Media (id: $id, type: ANIME) { id seasonYear } }";

/// Results per search.
const PER_PAGE: u32 = 5;

/// Largest edit distance for an English title. It allows for `Xth season`.
const FUZZY_EN: usize = 12;

/// Largest edit distance for a Japanese title.
const FUZZY_JA: usize = 5;

/// A search result: the id and the two titles that are compared.
#[derive(Debug, Deserialize)]
struct SearchMedia {
    id: i64,
    title: Option<MediaTitle>,
}

#[derive(Debug, Deserialize)]
struct MediaTitle {
    romaji: Option<String>,
    native: Option<String>,
}

/// `AniList` client for the title lookup. It holds the token and the transport,
/// and nothing else.
pub struct AniList<'a> {
    http: &'a dyn HttpClient,
    authorization: String,
}

impl<'a> AniList<'a> {
    /// Fails when no token is configured. An empty token counts as none, .
    pub fn new(http: &'a dyn HttpClient, token: Option<&str>) -> Result<Self> {
        let token = token
            .filter(|token| !token.is_empty())
            .ok_or(IdMapError::MissingAnilistToken)?;
        Ok(Self {
            http,
            authorization: format!("Bearer {token}"),
        })
    }

    fn post(&self, query: &str, variables: &Value) -> Result<Value> {
        let body = json!({ "query": query, "variables": variables });
        let headers = [
            ("Authorization", self.authorization.as_str()),
            ("Accept", "application/json"),
        ];
        Ok(self.http.post_json(ENDPOINT, &headers, &body)?)
    }

    /// The `data` of a reply, or `None` when the reply carries GraphQL errors.
    fn data(reply: &Value) -> Option<&Value> {
        if reply.get("errors").is_some_and(|errors| !errors.is_null()) {
            return None;
        }
        reply.get("data")
    }

    fn search(&self, term: &str) -> Result<Vec<SearchMedia>> {
        let reply = self.post(
            SEARCH_QUERY,
            &json!({ "search": term, "page": 1, "perPage": PER_PAGE }),
        )?;
        let Some(list) = Self::data(&reply)
            .and_then(|data| data.pointer("/Page/media"))
            .and_then(Value::as_array)
        else {
            return Ok(Vec::new());
        };
        list.iter()
            .map(|item| {
                serde_json::from_value(item.clone()).map_err(|source| IdMapError::Reply {
                    service: "AniList",
                    source,
                })
            })
            .collect()
    }

    /// The season year of one `AniList` entry. `None` when `AniList` has no year.
    fn season_year(&self, id: i64) -> Result<Option<i64>> {
        let reply = self.post(MEDIA_QUERY, &json!({ "id": id }))?;
        Ok(Self::data(&reply)
            .and_then(|data| data.pointer("/Media/seasonYear"))
            .and_then(Value::as_i64))
    }

    /// The `AniList` id that matches one of `titles`. An exact match wins over a
    /// fuzzy one.
    fn lookup(&self, titles: &[TitleVariant]) -> Result<Option<i64>> {
        let mut searched: Vec<TitleVariant> = titles
            .iter()
            .filter(|t| is_lookup_title(t))
            .cloned()
            .collect();
        // Year variants are searched after the plain titles. Their own year is
        // checked against each candidate's season year.
        let years: Vec<TitleVariant> = searched
            .iter()
            .filter(|t| t.year.is_none())
            .filter_map(year_variant)
            .collect();
        searched.extend(years);

        let mut exact: Option<i64> = None;
        let mut best: Option<(i64, usize)> = None;
        for title in &searched {
            if exact.is_some() {
                break;
            }
            let threshold = if title.language == "ja" {
                FUZZY_JA
            } else {
                FUZZY_EN
            };
            for media in self.search(&title.title)? {
                let name = media.title.as_ref().and_then(|names| {
                    if title.language == "ja" {
                        names.native.as_deref()
                    } else {
                        names.romaji.as_deref()
                    }
                });
                let Some(name) = name else {
                    continue;
                };

                if let Some(year) = title.year {
                    // A later exact match overwrites an earlier one within a reply,
                    // but a year match only ever improves the fuzzy one.
                    if self.season_year(media.id)? == Some(i64::from(year)) {
                        let distance = distance(&title.title, name);
                        if distance <= threshold && improves(best, distance) {
                            best = Some((media.id, distance));
                        }
                    }
                } else if name.to_lowercase() == title.title.to_lowercase() {
                    exact = Some(media.id);
                } else {
                    let distance = distance(&title.title, name);
                    if distance == 0 {
                        exact = Some(media.id);
                    } else if distance <= threshold && improves(best, distance) {
                        best = Some((media.id, distance));
                    }
                }
            }
        }

        Ok(exact.or(best.map(|(id, _)| id)))
    }
}

impl IdMapProvider for AniList<'_> {
    fn name(&self) -> &'static str {
        "anilist"
    }

    fn apply(&mut self, id: &mut AnimeId, titles: &[TitleVariant], overwrite: bool) -> Result<()> {
        if id.anilist.is_some() && !overwrite {
            return Ok(());
        }
        if let Some(found) = self.lookup(titles)? {
            id.anilist = Some(found);
        }
        Ok(())
    }
}

/// The official Japanese title and the main romaji title.
fn is_lookup_title(title: &TitleVariant) -> bool {
    matches!(
        (title.kind.as_deref(), title.language.as_str()),
        (Some("official"), "ja") | (Some("main"), "x-jat")
    )
}

/// The title without its year, for a title that has none of its own.
fn year_variant(title: &TitleVariant) -> Option<TitleVariant> {
    let (name, year) = split_year(&title.title)?;
    Some(TitleVariant {
        title: name.to_owned(),
        kind: title.kind.clone(),
        year: Some(year),
        language: title.language.clone(),
    })
}

/// Splits a trailing year in brackets off a title: one whitespace character,
/// `(`, exactly four ASCII digits, `)` at the very end, after a non-empty name.
///
/// - `Show (2013)` gives `Show` and 2013
/// - `Show: Part 2 (2021)` gives `Show: Part 2` and 2021
/// - `Show (20133)`, `Show (13)`, `Show(2013)`, `Show (2013) OVA` and ` (2013)`
///   have no year
fn split_year(title: &str) -> Option<(&str, i32)> {
    let (head, year) = title.strip_suffix(')')?.rsplit_once('(')?;
    if year.len() != 4 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let name = head.strip_suffix(char::is_whitespace)?;
    let year = year.parse().ok()?;
    (!name.is_empty()).then_some((name, year))
}

/// Collator distance between two titles, ignoring case.
fn distance(a: &str, b: &str) -> usize {
    text::levenshtein(&a.to_lowercase(), &b.to_lowercase())
}

/// True when `distance` is strictly better than the best so far.
fn improves(best: Option<(i64, usize)>, distance: usize) -> bool {
    best.is_none_or(|(_, score)| score > distance)
}

#[cfg(test)]
mod tests;
