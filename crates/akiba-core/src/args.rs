//! Argument parsers shared by the commands. They take a `&str` and return a
//! `Result<T, String>`, which is what a clap `value_parser` accepts, so this
//! crate needs no `clap`.

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::formatter::Template;
use crate::num::parse_i64;

/// Value parser for ids.
pub fn number_arg(value: &str) -> Result<i64, String> {
    parse_i64(value).ok_or_else(|| "Expecting a number.".to_owned())
}

/// The id stored for `blacklist`.
const BLACKLIST: i64 = -1;

/// Value parser for the id options that also take the marker: a number, or
/// exactly `blacklist`, which is stored as -1. The word must match as typed, so
/// `Blacklist` and padded spellings are rejected.
pub fn id_or_blacklist(value: &str) -> Result<i64, String> {
    if value == "blacklist" {
        return Ok(BLACKLIST);
    }
    parse_i64(value).ok_or_else(|| "Expecting a number or blacklist.".to_owned())
}

/// `path.resolve`: an absolute path without `.`, `..` and repeated or trailing
/// slashes. Symlinks are not resolved.
pub fn resolve(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().unwrap_or_default().join(path)
    };
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Value parser for `<path>` arguments.
#[allow(clippy::unnecessary_wraps)]
pub fn path_arg(value: &str) -> Result<PathBuf, String> {
    Ok(resolve(Path::new(value)))
}

/// Value parser for options that must name an existing directory.
pub fn dir_arg(value: &str) -> Result<PathBuf, String> {
    let path = resolve(Path::new(value));
    let is_dir = fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir());
    if is_dir {
        Ok(path)
    } else {
        Err(format!(
            "Path {} is no a directory or does not exist!",
            path.display()
        ))
    }
}

/// Value parser for format templates.
pub fn format_arg(value: &str) -> Result<String, String> {
    Template::parse(value).map_err(|e| e.to_string())?;
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_arg_is_strict() {
        assert_eq!(number_arg("12"), Ok(12));
        assert_eq!(number_arg(" +12 "), Ok(12));
        for bad in ["12x", "x", "", "-", "1.5"] {
            assert_eq!(
                number_arg(bad),
                Err("Expecting a number.".to_owned()),
                "{bad}"
            );
        }
    }

    #[test]
    fn id_or_blacklist_takes_a_number_or_the_marker() {
        assert_eq!(id_or_blacklist("12"), Ok(12));
        assert_eq!(id_or_blacklist("-1"), Ok(-1));
        assert_eq!(id_or_blacklist("blacklist"), Ok(-1));
    }

    #[test]
    fn id_or_blacklist_rejects_other_spellings() {
        for bad in ["Blacklist", "blacklisted", "", "junk", "x1", " blacklist "] {
            assert_eq!(
                id_or_blacklist(bad),
                Err("Expecting a number or blacklist.".to_owned()),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn resolve_normalises_lexically() {
        assert_eq!(resolve(Path::new("/a/b/")), Path::new("/a/b"));
        assert_eq!(resolve(Path::new("/a/./b/../c")), Path::new("/a/c"));
        assert_eq!(resolve(Path::new("/..")), Path::new("/"));
    }
}
