//! `ed2k` hashing and a single-pass multi-hash (`ed2k`, `crc32`, `md5`, `sha1`).
//!
//! The ed2k hash is MD4 over blocks of [`BLOCK_SIZE`] bytes. See
//! <https://wiki.anidb.net/Ed2k-hash>. A file of one block hashes to that block's
//! digest. Otherwise the hash is the MD4 of the concatenated block digests. A file
//! that is an exact multiple of the block size gets no trailing empty block.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use md4::{Digest, Md4};
use md5::Md5;
use sha1::Sha1;

/// The ed2k block size in bytes.
pub const BLOCK_SIZE: usize = 9_728_000;

/// Size of the read buffer used by [`hash_reader`].
const BUFFER_SIZE: usize = 1 << 20;

/// The hashes of one input. Hashes that were not selected are empty strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashes {
    /// Lowercase hex ed2k hash (MD4). Always computed.
    pub ed2k: String,
    /// Lowercase hex crc32 (IEEE), zero padded to 8 digits.
    pub crc32: String,
    /// Lowercase hex md5.
    pub md5: String,
    /// Lowercase hex sha1.
    pub sha1: String,
    /// Number of bytes hashed.
    pub size: u64,
}

/// Which hashes to compute besides ed2k, which is always computed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Select {
    /// Compute crc32.
    pub crc32: bool,
    /// Compute md5.
    pub md5: bool,
    /// Compute sha1.
    pub sha1: bool,
}

/// Incremental ed2k state. Blocks are finished lazily, so a full final block does
/// not produce an extra empty block.
#[derive(Default)]
struct Ed2k {
    blocks: Vec<[u8; 16]>,
    current: Md4,
    current_len: usize,
}

impl Ed2k {
    fn update(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            // a full block is only finished once more data arrives
            if self.current_len == BLOCK_SIZE {
                self.finish_block();
            }
            let take = data.len().min(BLOCK_SIZE - self.current_len);
            let (head, tail) = data.split_at(take);
            self.current.update(head);
            self.current_len += take;
            data = tail;
        }
    }

    fn finish_block(&mut self) {
        let digest = std::mem::take(&mut self.current).finalize();
        let mut block = [0u8; 16];
        block.copy_from_slice(&digest);
        self.blocks.push(block);
        self.current_len = 0;
    }

    fn finalize(mut self) -> String {
        if self.current_len > 0 {
            self.finish_block();
        }
        let digest = match self.blocks.as_slice() {
            [single] => *single,
            blocks => {
                // zero blocks (empty input) and several blocks both hash the
                // concatenated block digests
                let mut outer = Md4::new();
                for block in blocks {
                    outer.update(block);
                }
                let mut digest = [0u8; 16];
                digest.copy_from_slice(&outer.finalize());
                digest
            }
        };
        hex::encode(digest)
    }
}

/// Hash everything `r` yields, in one pass. ed2k is always computed, the others
/// only when selected.
///
/// # Errors
///
/// Returns the first read error other than `Interrupted`.
pub fn hash_reader<R: Read>(mut r: R, select: Select) -> io::Result<Hashes> {
    let mut ed2k = Ed2k::default();
    let mut crc = select.crc32.then(crc32fast::Hasher::new);
    let mut md5 = select.md5.then(Md5::new);
    let mut sha1 = select.sha1.then(Sha1::new);

    let mut size: u64 = 0;
    let mut buf = vec![0u8; BUFFER_SIZE];
    loop {
        let n = match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let chunk = &buf[..n];
        ed2k.update(chunk);
        if let Some(h) = crc.as_mut() {
            h.update(chunk);
        }
        if let Some(h) = md5.as_mut() {
            h.update(chunk);
        }
        if let Some(h) = sha1.as_mut() {
            h.update(chunk);
        }
        size += n as u64;
    }

    Ok(Hashes {
        ed2k: ed2k.finalize(),
        crc32: crc.map_or_else(String::new, |h| format!("{:08x}", h.finalize())),
        md5: md5.map_or_else(String::new, |h| hex::encode(h.finalize())),
        sha1: sha1.map_or_else(String::new, |h| hex::encode(h.finalize())),
        size,
    })
}

/// Hash the file at `path` in one pass. See [`hash_reader`].
///
/// # Errors
///
/// Returns an error if the file cannot be opened or read.
pub fn hash_file(path: &Path, select: Select) -> io::Result<Hashes> {
    hash_reader(File::open(path)?, select)
}

/// Format an `ed2k://|file|{name}|{size}|{hash}|/` link.
#[must_use]
pub fn ed2k_link(file_name: &str, size: u64, ed2k: &str) -> String {
    format!("ed2k://|file|{file_name}|{size}|{ed2k}|/")
}
