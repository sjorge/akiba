//! Anime mapping from `AniDB` metadata to the NFO types.
//!
//! The builders are pure: they return the data, and the caller writes it with
//! [`TvShowNfo`](super::TvShowNfo) or [`EpisodeNfo`](super::EpisodeNfo). The
//! poster is not part of the NFO. It is the `picture` of the show, fetched with
//! `anidb_http::fetch_poster` by the caller.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anidb_http::{AniDbShow, Episode as AniDbEpisode};

use super::{Episode, NfoError, TvShow, UniqueId};
use crate::anime::AnimeId;
use crate::num::parse_i64;

/// The id that marks a provider id as blacklisted in the legacy caches.
const BLACKLISTED: i64 = -1;

/// The episode range of a media file, and the title to use when `AniDB` has none
/// in English.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeFileInfo<'a> {
    /// Episode number as written in the file name, such as `1` or `S1`.
    pub episode_start: &'a str,
    pub episode_end: &'a str,
    pub title: &'a str,
}

/// The `tvshow.nfo` fields of an anime. An anime without titles is an error,
/// since the NFO would have an empty `<title>`.
pub fn tv_show(id: &AnimeId, metadata: &AniDbShow) -> Result<TvShow, NfoError> {
    // the first title is the fallback when there is no x-jat main title
    let first = metadata
        .titles
        .first()
        .ok_or(NfoError::NoTitles { aid: id.anidb })?;
    let mut show = TvShow {
        unique_ids: vec![UniqueId {
            id: id.anidb.to_string(),
            kind: "anidb".to_string(),
            default: true,
        }],
        title: replace_backticks(&first.title),
        ..TvShow::default()
    };

    // WARN: avoid tvdb, it causes issues with jellyfin
    for (kind, value) in [("anilist", id.anilist), ("tmdb", id.tmdb)] {
        if let Some(value) = value.filter(|&v| is_known(v)) {
            show.unique_ids.push(UniqueId {
                id: value.to_string(),
                kind: kind.to_string(),
                default: false,
            });
        }
    }

    // prefer the x-jat main title, and keep the official Japanese title
    for title in &metadata.titles {
        match (title.kind.as_deref(), title.language.as_str()) {
            (Some("main"), "x-jat") => show.title = replace_backticks(&title.title),
            (Some("official"), "ja") => {
                show.original_title = Some(replace_backticks(&title.title));
            }
            _ => {}
        }
    }

    show.premiered = premiered(&metadata.start_date);
    show.modified = modified_time(&metadata.start_date);
    // rating follows the Japanese system
    show.mpaa = Some(if metadata.age_restricted { "18+" } else { "G" }.to_string());
    Ok(show)
}

/// The `<episodedetails>` entries for one media file. A file can hold several
/// episodes, so every episode in the range that `AniDB` knows is returned, in
/// range order.
pub fn episodes(file: &EpisodeFileInfo<'_>, metadata: &AniDbShow) -> Vec<Episode> {
    let numbers = if file.episode_start == file.episode_end {
        vec![file.episode_start.to_string()]
    } else {
        expand_episode_range(file.episode_start, file.episode_end)
    };

    let mut episodes = Vec::new();
    for number in &numbers {
        for meta in metadata
            .episodes
            .iter()
            .filter(|meta| meta.episode_number == *number)
        {
            episodes.push(episode(file, meta));
        }
    }
    episodes
}

fn episode(file: &EpisodeFileInfo<'_>, meta: &AniDbEpisode) -> Episode {
    let mut episode = Episode {
        unique_ids: vec![UniqueId {
            id: meta.id.to_string(),
            kind: "anidb".to_string(),
            default: true,
        }],
        title: replace_backticks(file.title),
        ..Episode::default()
    };

    for title in &meta.titles {
        match title.language.as_str() {
            "en" => episode.title = replace_backticks(&title.title),
            "ja" => episode.original_title = Some(replace_backticks(&title.title)),
            _ => {}
        }
    }

    // regular episodes are season 1. Specials, credits and the like are season
    // 0, numbered from their type: type * 100 + n
    if meta.kind == 1 {
        episode.season = Some(1);
        episode.episode = parse_i64(&meta.episode_number);
    } else {
        episode.season = Some(0);
        episode.episode = parse_i64(after_first_char(&meta.episode_number))
            .and_then(|n| meta.kind.checked_mul(100)?.checked_add(n));
    }

    episode.premiered = premiered(&meta.air_date);
    episode.modified = modified_time(&meta.air_date);
    episode
}

/// `AniDB` dates are `YYYY-MM-DD`, `YYYY-MM` or empty. A bare month is the 1st.
fn premiered(date: &str) -> Option<String> {
    match date.encode_utf16().count() {
        10 => Some(date.to_string()),
        7 => Some(format!("{date}-01")),
        _ => None,
    }
}

/// The file time for an `AniDB` date: 12:00 UTC of the day, so the date reads
/// the same in every time zone.
///
/// The date is `YYYY-MM-DD`, `YYYY-MM` (the 1st) or `YYYY` (January 1st).
/// Anything else is `None`: empty, malformed, a month or day that does not
/// exist (leap years are honoured) and a date before 1970, which has no
/// `SystemTime` on every platform.
pub fn modified_time(date: &str) -> Option<SystemTime> {
    let number = |text: &str, width: usize| -> Option<i64> {
        if text.len() == width && text.bytes().all(|b| b.is_ascii_digit()) {
            text.parse().ok()
        } else {
            None
        }
    };
    let mut parts = date.split('-');
    let year = number(parts.next()?, 4)?;
    let month = parts.next().map_or(Some(1), |m| number(m, 2))?;
    let day = parts.next().map_or(Some(1), |d| number(d, 2))?;
    if parts.next().is_some() || year < 1970 || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=month_days).contains(&day) {
        return None;
    }
    let seconds = days_from_civil(year, month, day) * 86_400 + 12 * 3_600;
    Some(UNIX_EPOCH + Duration::from_secs(u64::try_from(seconds).ok()?))
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// algorithm), for `month` 1 to 12.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y.rem_euclid(400);
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// `AniDB` ids are kept unless they are the blacklist marker or zero (which
/// `if (id.anilist)` is treated as absent, so an id of 0 is not written).
fn is_known(id: i64) -> bool {
    id != 0 && id != BLACKLISTED
}

fn replace_backticks(title: &str) -> String {
    title.replace('`', "'")
}

/// The part of `s` after its first character, or an empty string.
fn after_first_char(s: &str) -> &str {
    s.char_indices().nth(1).map_or("", |(i, _)| &s[i..])
}

/// Expands `start..end` the way shell brace expansion does (`{1..3}`). It is used for
/// episode ranges such as `01-03`.
///
/// Integer bounds expand in either direction. Zero padding applies when a bound
/// has a leading zero, to the width of the longer bound. Single characters
/// expand as a character range. Anything else, such as `S1..S3`, is returned
/// as the literal `{start..end}`, as shell brace expansion does.
pub fn expand_episode_range(start: &str, end: &str) -> Vec<String> {
    let expanded = match (integer(start), integer(end)) {
        (Some(a), Some(b)) => expand_integers(start, end, a, b),
        _ => expand_characters(start, end),
    };
    if expanded.is_empty() {
        vec![format!("{{{start}..{end}}}")]
    } else {
        expanded
    }
}

/// `-?\d+`, the bounds that count as numbers.
fn integer(s: &str) -> Option<i64> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn expand_integers(start: &str, end: &str, a: i64, b: i64) -> Vec<String> {
    let width = if has_leading_zero(start) || has_leading_zero(end) {
        start.len().max(end.len())
    } else {
        0
    };
    let step = if a > b { -1 } else { 1 };
    let mut out = Vec::new();
    let mut n = a;
    loop {
        out.push(pad(n, width));
        if n == b {
            break;
        }
        n += step;
    }
    out
}

/// A number with its digits zero padded to `width`. The sign is not counted.
fn pad(n: i64, width: usize) -> String {
    if width == 0 {
        return n.to_string();
    }
    let sign = if n < 0 { "-" } else { "" };
    let digit_width = if n < 0 { width - 1 } else { width };
    format!("{sign}{:0>digit_width$}", n.unsigned_abs())
}

/// `-01` and `01` have a leading zero. `0` and `10` do not.
fn has_leading_zero(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    digits.len() > 1 && digits.starts_with('0')
}

/// Character ranges as shell brace expansion expands them. A bound that is neither an integer
/// nor a single character makes the whole range invalid. Only the first
/// character of an integer bound is used.
fn expand_characters(start: &str, end: &str) -> Vec<String> {
    if start.is_empty()
        || end.is_empty()
        || (integer(start).is_none() && start.chars().count() > 1)
        || (integer(end).is_none() && end.chars().count() > 1)
    {
        return Vec::new();
    }
    let (Some(first), Some(last)) = (start.chars().next(), end.chars().next()) else {
        return Vec::new();
    };
    let a = i64::from(u32::from(first));
    let b = i64::from(u32::from(last));
    let step = if a > b { -1 } else { 1 };
    let mut out = Vec::new();
    let mut code = a;
    loop {
        if let Some(ch) = u32::try_from(code).ok().and_then(char::from_u32) {
            out.push(ch.to_string());
        }
        if code == b {
            break;
        }
        code += step;
    }
    out
}

#[cfg(test)]
mod tests;
