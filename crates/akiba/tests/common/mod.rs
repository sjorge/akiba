//! Shared helpers: every test runs the real binary against its own temp dirs.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

/// A temp root with its own config and cache directories.
pub struct Env {
    pub root: TempDir,
}

impl Env {
    pub fn new() -> Self {
        Self {
            root: TempDir::new().expect("temp dir"),
        }
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.path().join("config").join("akiba.toml")
    }

    /// The akiba cache directory, as `XDG_CACHE_HOME` resolves it.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.path().join("cache").join("akiba")
    }

    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("akiba").expect("binary");
        // NOTE: the rest of the environment stays, the atomic writes stage in $TMPDIR
        cmd.env("HOME", self.root.path())
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("XDG_CACHE_HOME", self.root.path().join("cache"))
            .env("AKIBA_CONFIG", self.config_file());
        cmd
    }

    /// A directory below the temp root.
    pub fn dir(&self, name: &str) -> PathBuf {
        let path = self.root.path().join(name);
        std::fs::create_dir_all(&path).expect("create dir");
        path
    }
}

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn exists(path: &Path) -> bool {
    path.exists()
}
