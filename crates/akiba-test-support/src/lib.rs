//! Helpers for the tests of the `akiba-cmd-*` crates. Not published: the
//! command crates use it as a dev-dependency only.
//!
//! A command is driven in this process: its `Args` are parsed like the binary
//! does and its `run` is called with the logger captured. The environment is
//! process wide, so an [`Env`] holds a lock for its lifetime and the tests of
//! one binary take turns.

// NOTE: a test helper: a panic is how it fails the test, so no `# Panics` sections
#![allow(clippy::missing_panics_doc)]

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Mutex, MutexGuard, PoisonError};

use akiba_core::logger;
use clap::Args;
use tempfile::TempDir;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// What a command printed and how it ended.
pub struct Outcome {
    /// The exit code the binary would return: 1 for bad arguments.
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

/// A temp root with its own config and cache directories, set as `HOME`,
/// `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` and `AKIBA_CONFIG` until it is dropped.
pub struct Env {
    root: TempDir,
    saved: Vec<(&'static str, Option<OsString>)>,
    // NOTE: declared last, so the environment is restored before the lock is released
    _lock: MutexGuard<'static, ()>,
}

impl Env {
    pub fn new() -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let root = TempDir::new().expect("temp dir");
        let config_file = root.path().join("config").join("akiba.toml");
        let vars: [(&'static str, OsString); 4] = [
            ("HOME", root.path().into()),
            ("XDG_CONFIG_HOME", root.path().join("config").into()),
            ("XDG_CACHE_HOME", root.path().join("cache").into()),
            ("AKIBA_CONFIG", config_file.into()),
        ];
        let saved = vars
            .iter()
            .map(|(name, _)| (*name, env::var_os(name)))
            .collect();
        for (name, value) in &vars {
            env::set_var(name, value);
        }
        Self {
            root,
            saved,
            _lock: lock,
        }
    }

    pub fn root(&self) -> PathBuf {
        self.root.path().to_path_buf()
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.path().join("config").join("akiba.toml")
    }

    /// The akiba cache directory, as `XDG_CACHE_HOME` resolves it.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.path().join("cache").join("akiba")
    }

    /// A directory below the temp root.
    pub fn dir(&self, name: &str) -> PathBuf {
        let path = self.root.path().join(name);
        std::fs::create_dir_all(&path).expect("create dir");
        path
    }

    /// Parses `args` (without the program name) as the command's `Args` and
    /// runs it with the logger captured.
    pub fn run<A: Args>(&self, args: &[&str], run: fn(&A) -> ExitCode) -> Outcome {
        let command = A::augment_args(clap::Command::new("akiba-test"));
        let parsed = command
            .try_get_matches_from(std::iter::once("akiba-test").chain(args.iter().copied()))
            .and_then(|matches| A::from_arg_matches(&matches));
        let parsed = match parsed {
            Ok(parsed) => parsed,
            // like the binary: bad arguments end with 1, the message goes to stderr
            Err(err) => {
                return Outcome {
                    code: 1,
                    stdout: String::new(),
                    stderr: err.to_string(),
                }
            }
        };
        let mut code = None;
        let captured = logger::capture(false, || code = Some(run(&parsed)));
        Outcome {
            code: exit_status(code.expect("the command ran")),
            stdout: captured.stdout,
            stderr: captured.stderr,
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        for (name, value) in self.saved.drain(..).rev() {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }
}

/// `ExitCode` has no accessor, but the commands only return 0 and 1.
fn exit_status(code: ExitCode) -> u8 {
    if code == ExitCode::SUCCESS {
        0
    } else if code == ExitCode::FAILURE {
        1
    } else {
        panic!("unexpected exit code {code:?}")
    }
}
