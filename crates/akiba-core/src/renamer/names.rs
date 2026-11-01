//! Shortening of names the file system refuses as too long.

use std::io;

use regex::Regex;

use super::Result;
use crate::fileinfo::FileInfo;
use crate::formatter::Template;

/// The file name limit of ext4, APFS, ZFS and SMB, in bytes.
pub(super) const NAME_MAX: usize = 255;

/// How often a too long name is shortened before giving up.
pub(super) const MAX_SHORTENINGS: u32 = 2;

fn js_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Cut a name to `length` UTF-16 units, a cut name ends in `~`.
pub(super) fn truncate_name(name: &str, length: usize) -> String {
    if js_len(name) <= length {
        return name.to_owned();
    }
    let mut units = 0;
    let mut end = 0;
    for (index, ch) in name.char_indices() {
        units += ch.len_utf16();
        if units > length {
            break;
        }
        end = index + ch.len_utf8();
    }
    format!("{}~", name[..end].trim_end())
}

fn basename(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("")
}

/// Shorten the anime and episode names after a name was too long.
///
/// First try (`attempt` 0): find out which name is too long, the anime name is
/// swapped for its shortest short name, the episode names are cut to fit the
/// rest of the file name. When both are long, both are done at once. Second
/// try: the episode names become `Episode N`.
pub(super) fn shorten_names(
    data: &FileInfo,
    template: &Template,
    attempt: u32,
) -> Result<FileInfo> {
    let mut result = data.clone();
    if attempt > 0 {
        /// The leading zeros of every number that starts a word, keeping its
        /// last digit: `013` -> `13`, `0013-0014` -> `13-14`, `00` -> `0`.
        /// A number after a prefix, as in `S01`, is not at a word start and is
        /// kept. The `expect` cannot fail: the pattern is a fixed, valid literal.
        static LEADING_ZEROS: std::sync::LazyLock<Regex> =
            std::sync::LazyLock::new(|| Regex::new(r"\b0+(?<digit>[0-9])").expect("valid regex"));
        let name = format!(
            "Episode {}",
            LEADING_ZEROS.replace_all(&data.episode, "$digit")
        );
        result.episode_name.clone_from(&name);
        result.episode_name_romaji = name;
        return Ok(result);
    }

    // a name using over half of the file name is considered too long
    // NOTE: the kanji names are left alone, they are generally short already
    let anime_length = js_len(&data.anime_name_romaji).max(js_len(&data.anime_name_english));
    let episode_length = js_len(&data.episode_name).max(js_len(&data.episode_name_romaji));
    let mut anime_long = anime_length > NAME_MAX / 2;
    let mut episode_long = episode_length > NAME_MAX / 2;
    // neither stands out, blame the longest one
    if !anime_long && !episode_long {
        anime_long = anime_length > episode_length;
        episode_long = !anime_long;
    }

    if anime_long {
        let mut shorts: Vec<&String> = data
            .anime_name_short
            .iter()
            .filter(|name| !name.is_empty())
            .collect();
        shorts.sort_by_key(|name| js_len(name));
        if let Some(short) = shorts.first() {
            result.anime_name_romaji.clone_from(short);
            result.anime_name_english.clone_from(short);
        }
    }
    if episode_long {
        // room left in the file name once the episode names are removed
        // NOTE: tags are sanitized to ASCII, so a character is a byte
        let mut bare = result.clone();
        bare.episode_name.clear();
        bare.episode_name_romaji.clear();
        let rendered = template.render(&bare)?;
        let rest = basename(&rendered).len();
        let length = NAME_MAX.saturating_sub(rest + 1);
        result.episode_name = truncate_name(&result.episode_name, length);
        result.episode_name_romaji = truncate_name(&result.episode_name_romaji, length);
    }
    Ok(result)
}

/// True for the error of a name the file system refuses as too long.
pub(super) fn is_name_too_long(error: &io::Error) -> bool {
    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    const ENAMETOOLONG: i32 = 63;
    #[cfg(target_os = "linux")]
    const ENAMETOOLONG: i32 = 36;
    #[cfg(target_os = "illumos")]
    const ENAMETOOLONG: i32 = 78;
    #[cfg(not(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "illumos"
    )))]
    const ENAMETOOLONG: i32 = -1;

    error.kind() == io::ErrorKind::InvalidFilename || error.raw_os_error() == Some(ENAMETOOLONG)
}
