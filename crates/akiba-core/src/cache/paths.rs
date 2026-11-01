//! Cache file locations under a cache root. The root is the configured
//! `cache.path`, which can differ from the XDG default, so it is always passed in.

use std::io;
use std::path::{Path, PathBuf};

use crate::host_id;
use crate::logger::{self, Level};

/// `anidb/<aid>.aid.toml`
pub fn aid_file(root: &Path, aid: i64) -> PathBuf {
    root.join("anidb").join(format!("{aid}.aid.toml"))
}

/// `anidb/<ed2k>.fid.toml`
pub fn fid_file(root: &Path, ed2k: &str) -> PathBuf {
    root.join("anidb").join(format!("{ed2k}.fid.toml"))
}

/// `<sha256(hostname)>.hashes.toml`. Warns when the hostname is `localhost`,
/// because such a name is not unique between hosts.
pub fn hashes_file(root: &Path) -> io::Result<PathBuf> {
    let name = host_id::hostname()?;
    if host_id::is_localhost(&name) {
        logger::log(
            "hostname is localhost, which is not unique: the hash cache may be shared between hosts",
            Level::Warn,
        );
    }
    Ok(root.join(format!("{}.hashes.toml", host_id::hashed(&name))))
}

/// `titles.map.toml`
pub fn titles_file(root: &Path) -> PathBuf {
    root.join("titles.map.toml")
}

/// `list.map.toml`
pub fn list_file(root: &Path) -> PathBuf {
    root.join("list.map.toml")
}

/// `local.map.toml`
pub fn local_file(root: &Path) -> PathBuf {
    root.join("local.map.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_match_the_legacy_layout() {
        let root = Path::new("/cache");
        assert_eq!(
            aid_file(root, 10914),
            Path::new("/cache/anidb/10914.aid.toml")
        );
        assert_eq!(
            fid_file(root, "7dafd31d6d028e981d09301c5fbfc654"),
            Path::new("/cache/anidb/7dafd31d6d028e981d09301c5fbfc654.fid.toml")
        );
        assert_eq!(titles_file(root), Path::new("/cache/titles.map.toml"));
        assert_eq!(list_file(root), Path::new("/cache/list.map.toml"));
        assert_eq!(local_file(root), Path::new("/cache/local.map.toml"));
    }

    #[test]
    fn hashes_file_is_named_after_the_hostname_hash() {
        let root = Path::new("/cache");
        let name = host_id::hostname().expect("hostname");
        let expected = format!("{}.hashes.toml", host_id::hashed(&name));
        let path = hashes_file(root).expect("hashes file");
        assert_eq!(path, root.join(expected));
    }
}
