//! Config and cache locations. They are resolved from the environment on every
//! call, so tests can change the variables. The `dirs` crate is not used, since
//! it returns `~/Library` on macOS, and akiba keeps `~/.config` and `~/.cache`.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::PROG;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    #[error("HOME is not set, and no XDG override applies")]
    NoHome,
}

/// Reads a variable, treating an empty value as unset.
fn env_var(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

fn home_dir() -> Result<PathBuf, PathError> {
    env_var("HOME").map(PathBuf::from).ok_or(PathError::NoHome)
}

/// `$XDG_*_HOME/akiba`, or `$HOME/<fallback>/akiba` when the variable is unset.
fn xdg_dir(xdg_var: &str, fallback: &str) -> Result<PathBuf, PathError> {
    match env_var(xdg_var) {
        Some(base) => Ok(PathBuf::from(base).join(PROG)),
        None => Ok(home_dir()?.join(fallback).join(PROG)),
    }
}

fn config_dir() -> Result<PathBuf, PathError> {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

/// `$AKIBA_CONFIG`, else `config.toml` in the akiba config directory.
pub fn config_file() -> Result<PathBuf, PathError> {
    match env_var("AKIBA_CONFIG") {
        Some(file) => Ok(PathBuf::from(file)),
        None => Ok(config_dir()?.join("config.toml")),
    }
}

/// The akiba cache directory, from `$XDG_CACHE_HOME` or `$HOME/.cache`.
pub fn cache_dir() -> Result<PathBuf, PathError> {
    xdg_dir("XDG_CACHE_HOME", ".cache")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use serial_test::serial;

    use super::*;
    use crate::testutil::EnvGuard;

    /// Sets the four variables for the lifetime of the returned guard.
    fn vars(
        akiba: Option<&str>,
        xdg_config: Option<&str>,
        xdg_cache: Option<&str>,
        home: Option<&str>,
    ) -> EnvGuard {
        let value = |v: Option<&str>| v.map(OsString::from);
        EnvGuard::set(&[
            ("AKIBA_CONFIG", value(akiba)),
            ("XDG_CONFIG_HOME", value(xdg_config)),
            ("XDG_CACHE_HOME", value(xdg_cache)),
            ("HOME", value(home)),
        ])
    }

    #[test]
    #[serial]
    fn config_file_prefers_akiba_config_then_xdg_then_home() {
        {
            let _env = vars(Some("/etc/akiba.toml"), Some("/xdg"), None, Some("/home/u"));
            assert_eq!(config_file(), Ok(PathBuf::from("/etc/akiba.toml")));
        }
        {
            let _env = vars(None, Some("/xdg"), None, Some("/home/u"));
            assert_eq!(config_file(), Ok(PathBuf::from("/xdg/akiba/config.toml")));
        }
        {
            let _env = vars(None, None, None, Some("/home/u"));
            assert_eq!(
                config_file(),
                Ok(PathBuf::from("/home/u/.config/akiba/config.toml"))
            );
        }
    }

    #[test]
    #[serial]
    fn cache_dir_prefers_xdg_then_home() {
        {
            let _env = vars(None, None, Some("/xdg-cache"), Some("/home/u"));
            assert_eq!(cache_dir(), Ok(PathBuf::from("/xdg-cache/akiba")));
        }
        {
            let _env = vars(None, None, None, Some("/home/u"));
            assert_eq!(cache_dir(), Ok(PathBuf::from("/home/u/.cache/akiba")));
        }
    }

    #[test]
    #[serial]
    fn empty_variables_count_as_unset() {
        let _env = vars(Some(""), Some(""), Some(""), Some("/home/u"));
        assert_eq!(
            config_file(),
            Ok(PathBuf::from("/home/u/.config/akiba/config.toml"))
        );
        assert_eq!(cache_dir(), Ok(PathBuf::from("/home/u/.cache/akiba")));
    }

    #[test]
    #[serial]
    fn missing_home_is_an_error_without_an_override() {
        {
            let _env = vars(None, None, None, None);
            assert_eq!(config_file(), Err(PathError::NoHome));
            assert_eq!(cache_dir(), Err(PathError::NoHome));
        }
        {
            let _env = vars(Some("/etc/akiba.toml"), None, Some("/xdg-cache"), None);
            assert_eq!(config_file(), Ok(PathBuf::from("/etc/akiba.toml")));
            assert_eq!(cache_dir(), Ok(PathBuf::from("/xdg-cache/akiba")));
        }
    }
}
