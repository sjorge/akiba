// Fixture hash tests for the ed2k crate. Public API only; the expected values come from
// tests/fixtures/ed2k-hashes.json.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use ed2k::{ed2k_link, hash_file, hash_reader, Select};

// The size of the read buffer inside `hash_reader`, written out so that a trickle step equal
// to it is still tested without reaching into the crate.
const READ_BUFFER: usize = 1 << 20;

const ALL: Select = Select {
    crc32: true,
    md5: true,
    sha1: true,
};

/// byte(i) = (i * 31 + 7 + floor(i / 1048576)) & 0xff.
fn pattern(size: usize) -> Vec<u8> {
    (0..size as u64)
        .map(|i| ((i * 31 + 7 + i / 1_048_576) & 0xff) as u8)
        .collect()
}

fn fixture_hashes() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ed2k-hashes.json");
    let text = std::fs::read_to_string(path).expect("read tests/fixtures/ed2k-hashes.json");
    serde_json::from_str(&text).expect("parse tests/fixtures/ed2k-hashes.json")
}

/// Returns at most `step` bytes per read, to exercise block and buffer boundaries.
struct Trickle<'a> {
    data: &'a [u8],
    step: usize,
}

impl Read for Trickle<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = out.len().min(self.step).min(self.data.len());
        out[..n].copy_from_slice(&self.data[..n]);
        self.data = &self.data[n..];
        Ok(n)
    }
}

#[test]
fn matches_fixture_hashes() {
    let fixture = fixture_hashes();
    let files = fixture["files"].as_array().expect("files array");
    assert_eq!(files.len(), 5);
    for file in files {
        let size = usize::try_from(file["size"].as_u64().expect("size")).expect("size fits usize");
        let expected = (
            file["ed2k"].as_str().expect("ed2k"),
            file["crc32"].as_str().expect("crc32"),
            file["md5"].as_str().expect("md5"),
            file["sha1"].as_str().expect("sha1"),
        );
        let got = hash_reader(&pattern(size)[..], ALL).expect("hash");
        assert_eq!(got.size, size as u64, "size {size}");
        assert_eq!(
            (
                got.ed2k.as_str(),
                got.crc32.as_str(),
                got.md5.as_str(),
                got.sha1.as_str()
            ),
            expected,
            "size {size}"
        );
    }
}

#[test]
fn hash_file_matches_fixture_on_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("pattern.bin");
    std::fs::write(&path, pattern(9_728_001)).expect("write");

    let from_file = hash_file(&path, ALL).expect("hash_file");
    let from_slice = hash_reader(&pattern(9_728_001)[..], ALL).expect("hash_reader");
    assert_eq!(from_file, from_slice);
    assert_eq!(from_file.ed2k, "d9b8cd77af43021e4d86bd9d66accd0b");
}

#[test]
fn small_reads_match_one_read() {
    // Small on purpose: block boundaries are covered by tests/known_answers.rs.
    let data = pattern(70_001);
    let whole = hash_reader(&data[..], ALL).expect("hash");
    for step in [1, 4099, 65_535, READ_BUFFER] {
        let trickled = hash_reader(Trickle { data: &data, step }, ALL).expect("hash");
        assert_eq!(trickled, whole, "step {step}");
    }
}

#[test]
fn default_select_computes_only_ed2k() {
    let got = hash_reader(&b"abc"[..], Select::default()).expect("hash");
    assert_eq!(got.ed2k, "a448017aaf21d8525fc10ae87aa6729d");
    assert_eq!(got.size, 3);
    assert_eq!(got.crc32, "");
    assert_eq!(got.md5, "");
    assert_eq!(got.sha1, "");
}

#[test]
fn small_file_and_link() {
    let got = hash_reader(&b"abc"[..], ALL).expect("hash");
    assert_eq!(got.crc32, "352441c2");
    assert_eq!(got.md5, "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(got.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
    assert_eq!(
        ed2k_link("episode.mkv", got.size, &got.ed2k),
        "ed2k://|file|episode.mkv|3|a448017aaf21d8525fc10ae87aa6729d|/"
    );
}

#[test]
fn missing_file_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(hash_file(&dir.path().join("missing.mkv"), ALL).is_err());
}
