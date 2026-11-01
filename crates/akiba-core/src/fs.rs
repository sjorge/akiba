//! Atomic file writes. Readers see either the old or the new content, never a
//! truncated file.

use std::env;
use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Makes staged and temporary names unique within one process, so parallel
/// writers of the same basename never collide.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path` with `mode`. The data is staged in the system temp
/// directory, so a failure there never touches the target directory. It is then
/// copied next to `path`, chmodded, and renamed over it, which is atomic within
/// one directory. Both temporary files are removed on failure.
pub fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    write_atomic_at(path, bytes, mode, None)
}

/// [`write_atomic`] that also sets the modification time of the new file to
/// `modified`, when given. The time is set on the copy before the final rename,
/// which keeps it, so a reader never sees the new content with the wrong time.
pub fn write_atomic_at(
    path: &Path,
    bytes: &[u8],
    mode: u32,
    modified: Option<SystemTime>,
) -> io::Result<()> {
    write_atomic_in(&env::temp_dir(), path, bytes, mode, modified, |from, to| {
        fs::rename(from, to)
    })
}

/// Sets the modification time of an existing file. The file is opened read-only
/// on purpose: setting a time needs ownership, not write permission, so a
/// read-only file can be fixed too.
pub fn set_modified(path: &Path, modified: SystemTime) -> io::Result<()> {
    fs::File::open(path)?.set_modified(modified)
}

/// `write_atomic` with the staging directory and the rename injected, so tests
/// can make the rename fail.
fn write_atomic_in<R>(
    staging_dir: &Path,
    path: &Path,
    bytes: &[u8],
    mode: u32,
    modified: Option<SystemTime>,
    rename: R,
) -> io::Result<()>
where
    R: FnOnce(&Path, &Path) -> io::Result<()>,
{
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };

    let mut base = file_name.to_os_string();
    base.push(format!(
        ".{}.{}.tmp",
        process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let staged = RemoveOnDrop(staging_dir.join(&base));
    stage(&staged.0, bytes, mode)?;

    let tmp = dir.join(&base);
    let published = fs::copy(&staged.0, &tmp)
        .and_then(|_| fs::set_permissions(&tmp, Permissions::from_mode(mode)))
        .and_then(|()| match modified {
            Some(time) => set_modified(&tmp, time),
            None => Ok(()),
        })
        .and_then(|()| rename(&tmp, path));
    if published.is_err() {
        // best effort: the original error is more useful than a cleanup error
        let _ = fs::remove_file(&tmp);
    }
    published
}

/// Writes the staged file with the requested mode. `create_new` makes a name
/// clash an error instead of a silent overwrite.
fn stage(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    file.write_all(bytes)
}

/// Removes a temporary file when dropped, whatever the outcome.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        // best effort: the original error is more useful than a cleanup error
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{set_modified, write_atomic, write_atomic_at, write_atomic_in};

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn mode_of(path: &Path) -> u32 {
        fs::metadata(path).expect("metadata").permissions().mode() & 0o777
    }

    #[test]
    fn writes_content_and_leaves_no_tmp_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("cache.toml");

        write_atomic(&file, b"a = 1\n", 0o660).expect("write");

        assert_eq!(fs::read_to_string(&file).expect("read"), "a = 1\n");
        assert_eq!(names(dir.path()), ["cache.toml"]);
        assert_eq!(mode_of(&file), 0o660);
    }

    #[test]
    fn replaces_existing_file_and_tightens_its_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("config.toml");
        fs::write(&file, "old").expect("seed");
        fs::set_permissions(&file, Permissions::from_mode(0o644)).expect("chmod");

        write_atomic(&file, b"new", 0o600).expect("write");

        assert_eq!(fs::read_to_string(&file).expect("read"), "new");
        assert_eq!(mode_of(&file), 0o600);
    }

    #[test]
    fn keeps_old_file_and_cleans_up_when_rename_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let staging = tempfile::tempdir().expect("staging");
        let file = dir.path().join("cache.toml");
        fs::write(&file, "old").expect("seed");

        let result = write_atomic_in(staging.path(), &file, b"new", 0o660, None, |_, _| {
            Err(std::io::Error::other("EIO: input/output error"))
        });

        let err = result.expect_err("rename must fail");
        assert!(err.to_string().contains("EIO"));
        assert_eq!(fs::read_to_string(&file).expect("read"), "old");
        assert_eq!(names(dir.path()), ["cache.toml"]);
        assert!(
            names(staging.path()).is_empty(),
            "staged file must be removed"
        );
    }

    #[test]
    fn staging_is_removed_on_success() {
        let dir = tempfile::tempdir().expect("tempdir");
        let staging = tempfile::tempdir().expect("staging");
        let file = dir.path().join("cache.toml");

        write_atomic_in(staging.path(), &file, b"x", 0o600, None, |from, to| {
            std::fs::rename(from, to)
        })
        .expect("write");

        assert_eq!(names(staging.path()), Vec::<String>::new());
        assert_eq!(names(dir.path()), ["cache.toml"]);
    }

    fn mtime(path: &Path) -> SystemTime {
        fs::metadata(path)
            .expect("metadata")
            .modified()
            .expect("mtime")
    }

    #[test]
    fn write_atomic_at_sets_the_modification_time() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.nfo");
        let time = UNIX_EPOCH + Duration::from_secs(1_000_000_000);

        write_atomic_at(&file, b"x", 0o644, Some(time)).expect("write");

        assert_eq!(mtime(&file), time);
        assert_eq!(names(dir.path()), ["a.nfo"]);
    }

    #[test]
    fn write_atomic_at_replaces_a_file_and_its_time() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.nfo");
        fs::write(&file, "old").expect("seed");
        let time = UNIX_EPOCH + Duration::from_secs(1_000_000_000);

        write_atomic_at(&file, b"new", 0o644, Some(time)).expect("write");

        assert_eq!(fs::read_to_string(&file).expect("read"), "new");
        assert_eq!(mtime(&file), time);
    }

    #[test]
    fn write_atomic_without_a_time_uses_the_clock() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.nfo");

        write_atomic(&file, b"x", 0o644).expect("write");

        assert!(mtime(&file) > UNIX_EPOCH + Duration::from_secs(1_000_000_000));
    }

    #[test]
    fn set_modified_sets_the_time() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.nfo");
        fs::write(&file, "x").expect("seed");
        let time = UNIX_EPOCH + Duration::from_secs(1_000_000_000);

        set_modified(&file, time).expect("set");
        assert_eq!(mtime(&file), time);
        set_modified(&file, time + Duration::from_secs(5)).expect("set again");
        assert_eq!(mtime(&file), time + Duration::from_secs(5));
    }

    #[test]
    fn set_modified_works_on_a_read_only_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.nfo");
        fs::write(&file, "x").expect("seed");
        fs::set_permissions(&file, Permissions::from_mode(0o444)).expect("chmod");
        let time = UNIX_EPOCH + Duration::from_secs(1_000_000_000);

        set_modified(&file, time).expect("set");
        assert_eq!(mtime(&file), time);
    }

    #[test]
    fn path_without_file_name_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = write_atomic(&dir.path().join(".."), b"x", 0o600).expect_err("must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
