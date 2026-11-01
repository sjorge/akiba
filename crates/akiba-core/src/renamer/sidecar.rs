//! Sidecar files (subtitles and the like) that follow their episode.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::names::NAME_MAX;
use crate::anime::EPISODE_EXTENSIONS;

/// Extensions of the sidecar files that follow their episode, without the dot.
/// Matched case-insensitively. Add a format by adding its extension here.
const SIDECAR_EXTENSIONS: &[&str] = &[
    "nfo", "srt", "ass", "ssa", "vtt", "sub", "idx", "sup", "mks",
];

/// The file name of a sidecar of the episode `stem`: the stem and the `tail`
/// (`.nfo`, `.en.srt`, ...) that followed the stem on disk.
pub(super) fn sidecar_name(stem: &OsStr, tail: &OsStr) -> OsString {
    let mut name = stem.to_os_string();
    name.push(tail);
    name
}

/// [`sidecar_name`] for the destination. A name the file system refuses as too
/// long gets its stem cut and marked with `~`, the way episode names are cut.
/// The tail stays as it is.
pub(super) fn fit_sidecar_name(stem: &OsStr, tail: &OsStr) -> OsString {
    let name = sidecar_name(stem, tail);
    let Some(text) = stem.to_str().filter(|_| name.len() > NAME_MAX) else {
        return name;
    };
    let budget = NAME_MAX.saturating_sub(tail.len() + 1);
    let mut end = 0;
    for (index, ch) in text.char_indices() {
        if index + ch.len_utf8() > budget {
            break;
        }
        end = index + ch.len_utf8();
    }
    let mut fitted = OsString::from(format!("{}~", text[..end].trim_end()));
    fitted.push(tail);
    fitted
}

/// Whether `name` is `stem` followed by a `.`. `Show - 1.5` continues
/// `Show - 1`, `Show - 10` does not.
fn continues(name: &[u8], stem: &[u8]) -> bool {
    name.len() > stem.len() && name.starts_with(stem) && name[stem.len()] == b'.'
}

/// The tail of a sidecar named `name` for the episode `stem`: the part after
/// the stem, starting with its `.`, with the extension last. The tail is
/// `.srt`, `.en.srt`, `.eng.forced.ass`, and so on, and the extension must be
/// one of [`SIDECAR_EXTENSIONS`] (matched case-insensitively).
fn sidecar_tail<'a>(name: &'a [u8], stem: &[u8]) -> Option<&'a [u8]> {
    if !continues(name, stem) {
        return None;
    }
    let tail = &name[stem.len()..];
    let dot = tail.iter().rposition(|&byte| byte == b'.')?;
    let extension = &tail[dot + 1..];
    SIDECAR_EXTENSIONS
        .iter()
        .any(|known| extension.eq_ignore_ascii_case(known.as_bytes()))
        .then_some(tail)
}

/// Whether `name` has one of the [`EPISODE_EXTENSIONS`]. Unlike
/// [`crate::anime::is_episode_file`] this ignores case: a longer episode such as
/// `Show - 1.5.MKV` must still keep its subtitles away from `Show - 1`.
fn is_episode_name(name: &OsStr) -> bool {
    Path::new(name).extension().is_some_and(|extension| {
        EPISODE_EXTENSIONS
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    })
}

/// The tails (see [`sidecar_tail`]) of the sidecar files of the episode
/// `source`, as spelled on disk, sorted. The episode itself and directories
/// are skipped. A file that continues the stem of a longer episode next to it
/// is left to that episode: `Show - 1.5.srt` belongs to `Show - 1.5.mkv`, not
/// to `Show - 1.mkv`.
pub(super) fn find_sidecars(source: &Path, stem: &OsStr) -> Vec<OsString> {
    let Some(dir) = source.parent() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let files: Vec<OsString> = entries
        .filter_map(std::result::Result::ok)
        .filter(|entry| !entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name())
        .collect();
    let stem = stem.as_bytes();
    let longer: Vec<OsString> = files
        .iter()
        .filter(|name| is_episode_name(name))
        .filter_map(|name| Path::new(name).file_stem().map(OsStr::to_os_string))
        .filter(|episode| continues(episode.as_bytes(), stem))
        .collect();
    let mut found: Vec<OsString> = files
        .iter()
        .filter(|name| Some(name.as_os_str()) != source.file_name())
        .filter(|name| {
            !longer
                .iter()
                .any(|episode| continues(name.as_bytes(), episode.as_bytes()))
        })
        .filter_map(|name| sidecar_tail(name.as_bytes(), stem))
        .map(|tail| OsStr::from_bytes(tail).to_os_string())
        .collect();
    found.sort();
    found
}
