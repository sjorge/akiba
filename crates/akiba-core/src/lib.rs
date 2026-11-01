//! Shared akiba library: argument parsers, config, paths, cache, fs, logger, idmap, nfo and renamer.

pub mod anime;
pub mod args;
pub mod cache;
pub mod config;
pub mod fileinfo;
pub mod formatter;
pub mod fs;
pub mod host_id;
pub mod http;
pub mod idmap;
pub mod logger;
pub mod nfo;
pub mod num;
pub mod paths;
pub mod renamer;
pub mod resolver;
#[cfg(test)]
pub(crate) mod testutil;
pub mod text;

/// Program name, used for the config and cache directories.
pub const PROG: &str = "akiba";

/// Program version, taken from the crate manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
