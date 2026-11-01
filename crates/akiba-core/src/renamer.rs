//! The anime renamer: identifies episode files by their ed2k hash, looks them
//! up on `AniDB`, and moves, copies or links them to a path built from the
//! format template.
//!
//! The `AniDB` UDP client sits behind [`AnidbFiles`], so tests use a fake. The
//! real implementation, [`UdpFiles`], connects lazily and is not `Send`.

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::{symlink, DirBuilderExt};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anidb_udp::mylist::MylistState;
use anidb_udp::{MylistAdd, ResponseCode};

use crate::cache::paths::{fid_file, hashes_file};
use crate::cache::records::{HashRecord, HashesFile};
use crate::cache::{CacheError, CacheFile};
use crate::config::Config;
use crate::fileinfo::FileInfo;
use crate::formatter::{FormatError, Template};
use crate::fs::set_modified;

mod anidb;
mod names;
mod sidecar;

pub use anidb::{anime_fields, file_fields, file_info_from_reply, AnidbFiles, UdpFiles};
pub use anidb_udp::Error as AnidbError;

use names::{is_name_too_long, shorten_names, MAX_SHORTENINGS, NAME_MAX};
use sidecar::{find_sidecars, fit_sidecar_name, sidecar_name};

/// Mode of directories created by the renamer.
const DIR_MODE: u32 = 0o750;

/// Errors of the renamer.
#[derive(Debug, thiserror::Error)]
pub enum RenamerError {
    /// The configuration lacks what the renamer needs.
    #[error("Renamer requires at least config.anidb.udp_client to have name, username, and password set.")]
    Config,
    /// The file to identify does not exist.
    #[error("{} does not exist!", .0.display())]
    NotFound(PathBuf),
    /// The path to identify is not a regular file.
    #[error("{} must be a file!", .0.display())]
    NotAFile(PathBuf),
    /// A destination outside of the target directory.
    #[error("Refusing to rename {}: {} is outside of {}!", .source_path.display(), .destination.display(), .target.display())]
    OutsideTarget {
        source_path: PathBuf,
        destination: PathBuf,
        target: PathBuf,
    },
    /// A file system failure.
    #[error("{}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// A cache file failed.
    #[error(transparent)]
    Cache(#[from] CacheError),
    /// The format template is invalid, or cannot render the file.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// The `AniDB` client failed.
    #[error(transparent)]
    Anidb(#[from] AnidbError),
}

pub type Result<T> = std::result::Result<T, RenamerError>;

fn io_err(path: &Path, source: io::Error) -> RenamerError {
    RenamerError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// What to do with the source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Rename, falling back to copy and delete across file systems.
    Move,
    /// Copy, the source stays.
    Copy,
    /// Move, then leave a symlink to the destination at the source.
    Symlink,
}

/// What `rename` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Move,
    Copy,
    Symlink,
    /// The file is at its destination already.
    UpToDate,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Move => "move",
            Self::Copy => "copy",
            Self::Symlink => "symlink",
            Self::UpToDate => "uptodate",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarResult {
    /// Where the sidecar was.
    pub source: PathBuf,
    /// Where the sidecar goes, next to the renamed episode.
    pub destination: PathBuf,
    /// What was done (or would be done in a dry run).
    pub action: Outcome,
    /// Set when the sidecar could not be placed. The episode itself was
    /// renamed already and stays so.
    pub error: Option<String>,
}

/// An identified episode file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    /// Absolute path of the file.
    pub path: PathBuf,
    /// ed2k hash, size and link of the file.
    pub ed2k: HashRecord,
    /// The `AniDB` record, `None` for a hash-only identify or an unknown hash.
    pub data: Option<FileInfo>,
    /// Set by `rename`. Stays `None` when nothing was done.
    pub action: Option<Outcome>,
    /// Set by `rename`, when the file has a destination.
    pub destination_path: Option<PathBuf>,
    /// Set by `rename`: what happened to the sidecar files (`<stem>.nfo`,
    /// subtitles such as `<stem>.en.srt`) that sat next to the episode.
    pub sidecars: Vec<SidecarResult>,
}

/// Options of a [`Renamer`].
#[derive(Debug, Clone, Default)]
pub struct RenamerOptions {
    /// Target directory, defaults to `renamer.target_path` and then the cwd.
    pub target: Option<PathBuf>,
    /// Hash again, even when the hash cache has a fresh entry.
    pub rehash: bool,
    /// Format template, defaults to `renamer.format`.
    pub format: Option<String>,
    /// Replace an existing destination.
    pub overwrite: bool,
}

/// `path.resolve`: an absolute path with `.` and `..` folded away, without
/// touching the file system (symlinks are not resolved).
fn resolve(path: &Path) -> PathBuf {
    let mut out = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    };
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Gives a copy the modification time of its source. A plain move keeps it by
/// itself. Best effort: the file is placed either way.
fn keep_modified(source: &Path, destination: &Path) {
    if let Ok(time) = fs::metadata(source).and_then(|m| m.modified()) {
        let _ = set_modified(destination, time);
    }
}

fn key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
type RenameFn = fn(&Path, &Path) -> io::Result<()>;

/// Identifies, renames and tracks episode files.
pub struct Renamer {
    anidb: Box<dyn AnidbFiles>,
    template: Template,
    target: PathBuf,
    rehash: bool,
    overwrite: bool,
    cache_root: PathBuf,
    metadata_age: u32,
    hash_age: u32,
    hash_cache_file: CacheFile<HashesFile>,
    hash_cache: HashesFile,
    /// Hashes of this run, to fill hashes `AniDB` lacks without a second read.
    fresh_hashes: HashMap<PathBuf, ed2k::Hashes>,
    rename_fn: RenameFn,
}

impl Renamer {
    /// A renamer that talks to `AniDB` over UDP.
    ///
    /// # Errors
    ///
    /// Fails when the UDP client is not configured, the format is invalid or
    /// the hash cache cannot be read.
    pub fn new(config: &Config, options: RenamerOptions) -> Result<Self> {
        let files = UdpFiles::new(config)?;
        Self::with_client(config, options, Box::new(files))
    }

    /// A renamer over any [`AnidbFiles`], such as a fake in tests.
    ///
    /// # Errors
    ///
    /// Fails when the format is invalid or the hash cache cannot be read.
    #[allow(clippy::needless_pass_by_value)]
    pub fn with_client(
        config: &Config,
        options: RenamerOptions,
        anidb: Box<dyn AnidbFiles>,
    ) -> Result<Self> {
        let format = options.format.as_deref().unwrap_or(&config.renamer.format);
        let template = Template::parse(format)?;
        // NOTE: resolved, hash cache keys are built from the target
        let target = resolve(
            options
                .target
                .as_deref()
                .or(config.renamer.target_path.as_deref())
                .unwrap_or_else(|| Path::new(".")),
        );
        let cache_root = config.cache.path.clone();
        let hash_cache_file = CacheFile::new(
            hashes_file(&cache_root).map_err(|e| io_err(&cache_root, e))?,
            None,
        );
        let mut renamer = Self {
            anidb,
            template,
            target,
            rehash: options.rehash,
            overwrite: options.overwrite,
            cache_root,
            metadata_age: config.cache.metadata_age,
            hash_age: config.cache.hash_age,
            hash_cache_file,
            hash_cache: HashesFile::new(),
            fresh_hashes: HashMap::new(),
            rename_fn: |from, to| fs::rename(from, to),
        };
        renamer.read_hash_cache()?;
        Ok(renamer)
    }

    /// Replace (or keep) an existing destination on later renames.
    pub fn set_overwrite(&mut self, overwrite: bool) {
        self.overwrite = overwrite;
    }

    /// The resolved target directory.
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }

    fn read_hash_cache(&mut self) -> Result<()> {
        let Some(mut cache) = self.hash_cache_file.load_any_age()? else {
            return Ok(());
        };
        let before = cache.len();
        let now = now_millis();
        let max_age_ms = f64::from(self.hash_age) * 86_400_000.0;
        // NOTE: relative keys come from a relative target, before it was
        // resolved, they cannot be matched and are purged like missing files
        #[allow(clippy::cast_precision_loss)]
        cache.retain(|file, record| {
            let path = Path::new(file);
            path.is_absolute() && path.exists() && ((now - record.created_at) as f64) <= max_age_ms
        });
        if cache.len() != before {
            self.hash_cache_file.store(&cache)?;
        }
        self.hash_cache = cache;
        Ok(())
    }

    fn write_hash_cache(&self) -> Result<()> {
        Ok(self.hash_cache_file.store(&self.hash_cache)?)
    }

    fn fid_cache(&self, ed2k: &str, max_age: Option<u32>) -> CacheFile<FileInfo> {
        CacheFile::new(fid_file(&self.cache_root, ed2k), max_age)
    }

    /// Identify `path`: hash it (or take the hash cache), then look it up.
    ///
    /// # Errors
    ///
    /// Fails for a missing path, a cache or I/O failure, or an `AniDB` error
    /// other than an unknown hash (which gives `data: None`).
    pub fn identify(&mut self, path: &Path) -> Result<Episode> {
        self.identify_with(path, false, false)
    }

    /// Hash `path` only, no lookup.
    ///
    /// # Errors
    ///
    /// Fails for a missing path, or a cache or I/O failure.
    pub fn hash_only(&mut self, path: &Path) -> Result<Episode> {
        self.identify_with(path, false, true)
    }

    /// [`Renamer::identify`] with `refresh` (skip the cached `AniDB` record) and
    /// `hash_only` (skip the lookup).
    ///
    /// # Errors
    ///
    /// See [`Renamer::identify`].
    pub fn identify_with(
        &mut self,
        path: &Path,
        refresh: bool,
        hash_only: bool,
    ) -> Result<Episode> {
        // NOTE: the hash cache is keyed by absolute path, library callers may
        // pass a relative one
        let path = resolve(path);
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(RenamerError::NotFound(path));
            }
            Err(e) => return Err(io_err(&path, e)),
        };
        if !metadata.is_file() {
            return Err(RenamerError::NotAFile(path));
        }

        let path_key = key(&path);
        let stale = self.rehash
            || self
                .hash_cache
                .get(&path_key)
                .is_none_or(|record| record.size != metadata.len());
        if stale {
            // one pass for everything: AniDB may lack crc32, md5 or sha1
            let select = ed2k::Select {
                crc32: true,
                md5: true,
                sha1: true,
            };
            let hashes = ed2k::hash_file(&path, select).map_err(|e| io_err(&path, e))?;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            let record = HashRecord {
                link: ed2k::ed2k_link(
                    name.as_deref().unwrap_or_default(),
                    hashes.size,
                    &hashes.ed2k,
                ),
                hash: hashes.ed2k.clone(),
                size: hashes.size,
                created_at: now_millis(),
            };
            self.hash_cache.insert(path_key.clone(), record);
            self.fresh_hashes.insert(path.clone(), hashes);
            self.write_hash_cache()?;
        }
        let record = self.hash_cache[&path_key].clone();

        if hash_only {
            return Ok(Episode {
                path,
                ed2k: record,
                data: None,
                action: None,
                destination_path: None,
                sidecars: Vec::new(),
            });
        }

        let cache = self.fid_cache(&record.hash, Some(self.metadata_age));
        let mut data = if refresh { None } else { cache.load()? };
        if data.is_none() {
            data = self.lookup(&path, &record)?;
            if let Some(info) = &data {
                self.fid_cache(&record.hash, None).store(info)?;
            }
        }

        Ok(Episode {
            path,
            ed2k: record,
            data,
            action: None,
            destination_path: None,
            sidecars: Vec::new(),
        })
    }

    /// FILE by hash, with the hashes `AniDB` lacks filled in from the file.
    fn lookup(&mut self, path: &Path, record: &HashRecord) -> Result<Option<FileInfo>> {
        let mut info = match self.anidb.file_by_hash(&record.hash, record.size) {
            Ok(info) => info,
            // NO_SUCH_FILE
            Err(e) if e.code() == Some(ResponseCode::NoSuchFile) => return Ok(None),
            Err(e) => {
                self.anidb.disconnect();
                return Err(e.into());
            }
        };
        if info.crc32.is_empty() || info.sha1.is_empty() || info.md5.is_empty() {
            let hashes = match self.fresh_hashes.get(path) {
                Some(hashes) if hashes.ed2k == record.hash => hashes.clone(),
                _ => {
                    let select = ed2k::Select {
                        crc32: true,
                        md5: true,
                        sha1: true,
                    };
                    ed2k::hash_file(path, select).map_err(|e| io_err(path, e))?
                }
            };
            info.crc32 = hashes.crc32;
            info.md5 = hashes.md5;
            info.sha1 = hashes.sha1;
        }
        Ok(Some(info))
    }

    /// Move, copy or link `episode` to its destination. Without `AniDB` data
    /// nothing happens. With `dry_run` nothing is touched, but the outcome and
    /// destination are set.
    ///
    /// A name too long for the file system is shortened and retried, see
    /// `shorten_names`.
    ///
    /// # Errors
    ///
    /// Fails when the destination would be outside the target, on a template
    /// failure, and on any file system error other than a too long name.
    pub fn rename(&mut self, episode: &mut Episode, action: Action, dry_run: bool) -> Result<()> {
        let Some(mut data) = episode.data.clone() else {
            return Ok(());
        };
        let source = episode.path.clone();
        let copy = action == Action::Copy;

        let mut attempt = 0;
        let destination = loop {
            let rendered = self.template.render(&data)?;
            let destination = resolve(&self.target.join(rendered.trim_start_matches('/')));

            // NOTE: the formatted name must stay inside the target, tag values
            // come from remote data
            let inside = destination
                .strip_prefix(&self.target)
                .is_ok_and(|relative| !relative.as_os_str().is_empty());
            if !inside {
                return Err(RenamerError::OutsideTarget {
                    source_path: source,
                    destination,
                    target: self.target.clone(),
                });
            }

            // file already correct path
            if source == destination {
                episode.destination_path = Some(destination);
                episode.action = Some(Outcome::UpToDate);
                return Ok(());
            }

            // keep an existing destination without overwrite
            if destination.exists() && !self.overwrite {
                return Ok(());
            }

            // NOTE: a name that is too long for the file system is not known
            // upfront, so try, and on ENAMETOOLONG retry with shorter names
            match self.place(&source, &destination, copy, dry_run) {
                Ok(()) => break destination,
                Err(e) if is_name_too_long(&e) && attempt < MAX_SHORTENINGS => {
                    data = shorten_names(&data, &self.template, attempt)?;
                    attempt += 1;
                }
                Err(e) => return Err(io_err(&destination, e)),
            }
        };

        episode.destination_path = Some(destination.clone());
        episode.action = Some(if copy { Outcome::Copy } else { Outcome::Move });
        if action == Action::Symlink {
            if !dry_run {
                symlink(&destination, &source).map_err(|e| io_err(&source, e))?;
            }
            episode.action = Some(Outcome::Symlink);
        }

        // update hashes
        if !dry_run {
            let mut record = episode.ed2k.clone();
            let name = destination
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            record.link = ed2k::ed2k_link(
                name.as_deref().unwrap_or_default(),
                record.size,
                &record.hash,
            );
            if !copy {
                self.hash_cache.remove(&key(&source));
            }
            self.hash_cache.insert(key(&destination), record.clone());
            episode.ed2k = record;
            self.write_hash_cache()?;
        }

        // NOTE: after the hash cache, a sidecar problem must not hide that the
        // episode was renamed
        episode.sidecars = self.place_sidecars(&source, &destination, action, dry_run);
        Ok(())
    }

    /// Moves, copies or links the sidecars of `source` next to `destination`,
    /// the same way as the episode. A failure is kept in the result.
    fn place_sidecars(
        &self,
        source: &Path,
        destination: &Path,
        action: Action,
        dry_run: bool,
    ) -> Vec<SidecarResult> {
        let (Some(source_stem), Some(destination_stem)) =
            (source.file_stem(), destination.file_stem())
        else {
            return Vec::new();
        };
        let outcome = match action {
            Action::Move => Outcome::Move,
            Action::Copy => Outcome::Copy,
            Action::Symlink => Outcome::Symlink,
        };
        let mut results = Vec::new();
        for tail in find_sidecars(source, source_stem) {
            let sidecar_source = source.with_file_name(sidecar_name(source_stem, &tail));
            let sidecar_destination =
                destination.with_file_name(fit_sidecar_name(destination_stem, &tail));
            if sidecar_source == sidecar_destination {
                continue;
            }
            // keep an existing destination without overwrite
            if sidecar_destination.exists() && !self.overwrite {
                continue;
            }
            let mut result = SidecarResult {
                source: sidecar_source.clone(),
                destination: sidecar_destination.clone(),
                action: outcome,
                error: None,
            };
            let placed = self
                .place(
                    &sidecar_source,
                    &sidecar_destination,
                    action == Action::Copy,
                    dry_run,
                )
                .and_then(|()| {
                    if action == Action::Symlink && !dry_run {
                        symlink(&sidecar_destination, &sidecar_source)
                    } else {
                        Ok(())
                    }
                });
            if let Err(e) = placed {
                result.error = Some(io_err(&sidecar_destination, e).to_string());
            }
            results.push(result);
        }
        results
    }

    /// Creates the parent directory and copies or moves the file.
    fn place(
        &self,
        source: &Path,
        destination: &Path,
        copy: bool,
        dry_run: bool,
    ) -> io::Result<()> {
        // a component over NAME_MAX bytes fails on every file system we know,
        // saying so up front also makes a dry run shorten names
        let relative = destination
            .strip_prefix(&self.target)
            .unwrap_or(destination);
        if relative
            .components()
            .any(|c| c.as_os_str().len() > NAME_MAX)
        {
            return Err(io::Error::from(io::ErrorKind::InvalidFilename));
        }
        if dry_run {
            return Ok(());
        }
        if let Some(parent) = destination.parent() {
            DirBuilder::new()
                .recursive(true)
                .mode(DIR_MODE)
                .create(parent)?;
        }
        if copy {
            // NOTE: fs::copy overwrites by default
            fs::copy(source, destination)?;
            keep_modified(source, destination);
            return Ok(());
        }
        match (self.rename_fn)(source, destination) {
            Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
                // fallback to copy for filesystem boundaries
                fs::copy(source, destination)?;
                keep_modified(source, destination);
                fs::remove_file(source)
            }
            other => other,
        }
    }

    /// Set the mylist storage state of `episode`, adding it to the mylist when
    /// it is not there. Returns false when `AniDB` did not confirm.
    ///
    /// # Errors
    ///
    /// Fails on an `AniDB` error other than 321 (no such entry).
    pub fn set_mylist_state(&mut self, episode: &Episode, state: MylistState) -> Result<bool> {
        let (hash, size) = (episode.ed2k.hash.as_str(), episode.ed2k.size);
        let (need_update, edit) = match self.anidb.mylist_by_hash(hash, size) {
            Ok(entry) => (entry.state != Some(state), true),
            // NO_SUCH_ENTRY
            Err(e) if e.code() == Some(ResponseCode::NoSuchEntry) => (true, false),
            Err(e) => {
                self.anidb.disconnect();
                return Err(e.into());
            }
        };
        if need_update {
            let code = self.anidb.mylist_add_by_hash(
                hash,
                size,
                MylistAdd {
                    edit,
                    state: Some(state),
                },
            )?;
            if code != ResponseCode::MylistEntryAdded && code != ResponseCode::MylistEntryEdited {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Close the `AniDB` session.
    pub fn destroy(&mut self) {
        self.anidb.disconnect();
    }
}

impl fmt::Debug for Renamer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Renamer")
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
