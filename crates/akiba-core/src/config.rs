//! The `config.toml` model. Keys and defaults are unchanged, so a `config.toml`
//! written by akiba 2.x loads as it is.
//!
//! Every table has `#[serde(default)]`, so a partial table keeps the defaults of
//! its missing keys, so an old or partial file still loads. Unknown keys are kept in
//! `extra` and written back on save, so hand-added keys survive.

use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use anidb_http::TlsOptions;
use serde::{Deserialize, Serialize};

use crate::fs::write_atomic;
use crate::paths::{self, PathError};

const DEFAULT_HTTP_URL: &str = "http://api.anidb.net:9001/httpapi";
const DEFAULT_FORMAT: &str =
    "{anime_name_romaji} - {episode} - {episode_name} ({crc32}).{filetype}";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("cannot read config {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot parse config {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("cannot serialise config: {0}")]
    Serialise(#[from] toml::ser::Error),
    #[error("cannot write config {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// The config could not be loaded. Every command that loads the config fails
/// with this, so the logged text is the same in each of them.
#[derive(Debug, thiserror::Error)]
#[error("Failed to read the configuration")]
pub struct ConfigLoadError(#[source] pub ConfigError);

impl From<ConfigError> for ConfigLoadError {
    fn from(error: ConfigError) -> Self {
        Self(error)
    }
}

/// The commands that need configuration. Each one checks its own keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    WriteNfo,
    Renamer,
}

/// One missing setting. `message` is the text that is logged, and
/// each problem is logged as its own error line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigProblem {
    pub message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub anidb: AnidbConfig,
    pub anilist: AnilistConfig,
    pub tmdb: TmdbConfig,
    pub cache: CacheConfig,
    pub tls: TlsConfig,
    pub renamer: RenamerConfig,
    pub overwrite_nfo: bool,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnidbConfig {
    pub http_client: HttpClientConfig,
    pub udp_client: UdpClientConfig,
    pub poster: bool,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpClientConfig {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_HTTP_URL.to_owned(),
            name: None,
            version: None,
            extra: toml::Table::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UdpClientConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnilistConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TmdbConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

/// `[tls]`: where the trusted root certificates come from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsConfig {
    /// A PEM file of CA certificates. When set (and not empty), only these are
    /// trusted and the system trust store is not searched. Unset or empty means
    /// automatic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ca_file: Option<PathBuf>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Empty until `Config::load_from` fills in the XDG cache directory.
    pub path: PathBuf,
    /// Ages are in days.
    pub metadata_age: u32,
    pub mapping_age: u32,
    pub title_age: u32,
    pub hash_age: u32,
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            metadata_age: 90,
            mapping_age: 7,
            title_age: 3,
            hash_age: 30,
            extra: toml::Table::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenamerConfig {
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_path: Option<PathBuf>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Default for RenamerConfig {
    fn default() -> Self {
        Self {
            format: DEFAULT_FORMAT.to_owned(),
            target_path: None,
            extra: toml::Table::new(),
        }
    }
}

impl Default for Config {
    /// The default configuration. `cache.path` is left empty here,
    /// and `load_from` fills it in.
    fn default() -> Self {
        Self {
            anidb: AnidbConfig::default(),
            anilist: AnilistConfig::default(),
            tmdb: TmdbConfig::default(),
            cache: CacheConfig::default(),
            tls: TlsConfig::default(),
            renamer: RenamerConfig::default(),
            overwrite_nfo: false,
            extra: toml::Table::new(),
        }
    }
}

impl Config {
    /// Loads the config from `paths::config_file()`, or the defaults when the
    /// file does not exist.
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from(&paths::config_file()?)
    }

    /// Loads the config from `path`, or the defaults when it is not a file.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let mut config = if path.is_file() {
            let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
                path: path.to_path_buf(),
                source,
            })?;
            toml::from_str::<Self>(&text).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?
        } else {
            Self::default()
        };
        if config.cache.path.as_os_str().is_empty() {
            config.cache.path = paths::cache_dir()?;
        }
        Ok(config)
    }

    /// Saves the config to `paths::config_file()`.
    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&paths::config_file()?)
    }

    /// Saves the config to `path`. The directory is created with mode 0750 if
    /// it is missing, and the file is written with mode 0600, even when it
    /// already existed with looser permissions.
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string(self)?;
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            if !dir.exists() {
                DirBuilder::new()
                    .recursive(true)
                    .mode(0o750)
                    .create(dir)
                    .map_err(|source| ConfigError::Write {
                        path: dir.to_path_buf(),
                        source,
                    })?;
            }
        }
        write_atomic(path, text.as_bytes(), 0o600).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    /// The TLS settings every HTTP client is built from.
    #[must_use]
    pub fn tls_options(&self) -> TlsOptions {
        TlsOptions::new(self.tls.ca_file.clone())
    }

    /// Checks the keys `command` needs. An empty list means the config is usable.
    pub fn validate(&self, command: Command) -> Result<(), Vec<ConfigProblem>> {
        let mut problems = Vec::new();
        let mut push = |message| problems.push(ConfigProblem { message });

        match command {
            Command::WriteNfo => {
                // NOTE: the URL is a string with a default, so an empty value is
                // the way it counts as not configured.
                if self.anidb.http_client.url.is_empty() {
                    push("Please configure the AniDB URL!");
                }
                let http = &self.anidb.http_client;
                if http.name.is_none() || http.version.is_none() {
                    push("AniDB HTTP Client not configured! Please set with --anidb-http-client");
                    push("Please register a project on https://anidb.net/software/add, and add a HTTP client");
                }
            }
            Command::Renamer => {
                let udp = &self.anidb.udp_client;
                if udp.name.is_none() || udp.version.is_none() {
                    push("AniDB UDP Client not configured! Please set with --anidb-udp-client.");
                    push("Please register a project on https://anidb.net/software/add, and add a UDP client");
                }
                if udp.username.is_none() || udp.password.is_none() {
                    push("AniDB UDP Client missing username and password configuration! Please set with --anidb-auth-username and --anidb-auth-password.");
                }
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }
}

#[cfg(test)]
mod tests;
