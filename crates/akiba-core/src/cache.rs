//! Generic TOML cache files. Every cache file goes through [`CacheFile`]: the
//! age check, the atomic write and the legacy read.

use std::collections::BTreeMap;
use std::fs::{self, DirBuilder};
use std::io;
use std::marker::PhantomData;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::fs::write_atomic;

pub mod paths;
pub mod records;

const DIR_MODE: u32 = 0o750;
const FILE_MODE: u32 = 0o660;
const SECS_PER_DAY: f64 = 86_400.0;

/// Errors from reading or writing a cache file. Every variant names the file.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("{}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{}: invalid cache file: {source}", .path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("{}: cannot serialise cache: {source}", .path.display())]
    Serialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
}

pub type Result<T> = std::result::Result<T, CacheError>;

/// A map keyed by integer ids, such as the `AniDB` id in `titles.map.toml`.
///
/// TOML table keys are strings, so the keys are written as decimal strings and
/// parsed back on read. A non-integer key is a parse error.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyedMap<V>(pub BTreeMap<i64, V>);

impl<V: Serialize> Serialize for KeyedMap<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(key, value)| (key.to_string(), value)))
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for KeyedMap<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let raw = BTreeMap::<String, V>::deserialize(deserializer)?;
        let mut map = BTreeMap::new();
        for (key, value) in raw {
            let id = key.parse::<i64>().map_err(|_| {
                <D::Error as serde::de::Error>::custom(format!(
                    "cache key `{key}` is not an integer"
                ))
            })?;
            map.insert(id, value);
        }
        Ok(Self(map))
    }
}

/// One cache file of type `T`, with an optional maximum age in days.
#[derive(Debug, Clone)]
pub struct CacheFile<T> {
    path: PathBuf,
    max_age_days: Option<u32>,
    _t: PhantomData<T>,
}

impl<T> CacheFile<T> {
    /// `max_age_days: None` means the file never expires.
    pub fn new(path: impl Into<PathBuf>, max_age_days: Option<u32>) -> Self {
        Self {
            path: path.into(),
            max_age_days,
            _t: PhantomData,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl<T: Serialize + DeserializeOwned> CacheFile<T> {
    /// True when the file exists and its mtime is younger than the maximum age.
    /// The age is fractional days. An mtime in the future counts as age zero.
    pub fn is_fresh(&self) -> bool {
        let Ok(metadata) = fs::metadata(&self.path) else {
            return false;
        };
        let Some(max_age) = self.max_age_days else {
            return true;
        };
        let Ok(modified) = metadata.modified() else {
            return false;
        };
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::ZERO);
        age.as_secs_f64() / SECS_PER_DAY < f64::from(max_age)
    }

    /// The value, or `None` when the file is missing or stale.
    pub fn load(&self) -> Result<Option<T>> {
        if !self.is_fresh() {
            return Ok(None);
        }
        self.load_any_age()
    }

    /// The value, or `None` when the file is missing. Age is ignored.
    pub fn load_any_age(&self) -> Result<Option<T>> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(self.io_error(source)),
        };
        toml::from_str::<T>(&text)
            .map(Some)
            .map_err(|e| self.parse_error(e))
    }

    /// Writes the value atomically with mode 0660, creating the parent directory
    /// with mode 0750 when it is missing.
    pub fn store(&self, value: &T) -> Result<()> {
        let body = toml::to_string(value).map_err(|source| CacheError::Serialize {
            path: self.path.clone(),
            source,
        })?;
        if let Some(dir) = self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            DirBuilder::new()
                .recursive(true)
                .mode(DIR_MODE)
                .create(dir)
                .map_err(|source| self.io_error(source))?;
        }
        write_atomic(&self.path, body.as_bytes(), FILE_MODE).map_err(|source| self.io_error(source))
    }

    fn io_error(&self, source: io::Error) -> CacheError {
        CacheError::Io {
            path: self.path.clone(),
            source,
        }
    }

    fn parse_error(&self, source: toml::de::Error) -> CacheError {
        CacheError::Parse {
            path: self.path.clone(),
            source,
        }
    }
}

#[cfg(test)]
mod tests;
