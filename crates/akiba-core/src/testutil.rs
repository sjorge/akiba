//! Helpers for the crate's unit tests. Tests that use them must be `#[serial]`,
//! because the environment is process wide.

use std::env;
use std::ffi::OsString;
use std::path::Path;

use tempfile::TempDir;

/// Sets environment variables for its lifetime and restores the previous values
/// on drop. `None` removes a variable.
pub(crate) struct EnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    pub(crate) fn set(vars: &[(&'static str, Option<OsString>)]) -> Self {
        let saved = vars
            .iter()
            .map(|(name, _)| (*name, env::var_os(name)))
            .collect();
        for (name, value) in vars {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in self.saved.drain(..).rev() {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }
}

/// A temporary root that holds config, cache and `HOME`.
pub(crate) struct TempEnv {
    _guard: EnvGuard,
    root: TempDir,
}

impl TempEnv {
    pub(crate) fn new() -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let config = root.path().join("config");
        let cache = root.path().join("cache");
        let guard = EnvGuard::set(&[
            ("HOME", Some(root.path().as_os_str().to_owned())),
            ("XDG_CONFIG_HOME", Some(config.as_os_str().to_owned())),
            ("XDG_CACHE_HOME", Some(cache.as_os_str().to_owned())),
            (
                "AKIBA_CONFIG",
                Some(config.join("akiba.toml").into_os_string()),
            ),
        ]);
        Self {
            _guard: guard,
            root,
        }
    }

    pub(crate) fn root(&self) -> &Path {
        self.root.path()
    }
}
