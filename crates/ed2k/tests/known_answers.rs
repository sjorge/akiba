// Known-answer tests for the ed2k hash. Public API only.
//
// Every expected value is a string literal. None of them comes from this crate or
// from tests/fixtures/ed2k-hashes.json. Each group below says how its values were computed.
//
// Input: `pattern(n)` is n bytes where byte i is i % 251. The same bytes were written
// to files outside the repository and hashed there.
//
// Reference tool: OpenSSL 4.1 (Homebrew, /opt/homebrew/bin/openssl) with the legacy
// provider for MD4:
//     openssl dgst -provider legacy -provider default -md4
// It reproduces the RFC 1320 vectors: MD4("") = 31d6cfe0..., MD4("a") = bde52cb3...,
// MD4("abc") = a448017a.... Its block-digest pipeline reproduces the ed2k-hashes.json values
// for 9_728_001 bytes (d9b8cd77...) and 19_456_000 bytes (1a6dbd09...) of the fixture
// pattern (byte i = (i * 31 + 7 + floor(i / 1048576)) & 0xff) of tests/fixture_hashes.rs.
//
// Multi-block values: `split -b 9728000` the file, `openssl dgst -md4 -binary` on each
// piece, concatenate the raw binary digests (not their hex text), and hash that with
// `openssl dgst -md4`.

use std::io::{self, Read};

use ed2k::{ed2k_link, hash_reader, Select};
use md4::{Digest, Md4};

// Written out here, not taken from the crate, so that a changed constant fails the tests.
const BLOCK: usize = 9_728_000;

// MD4 of the empty input, RFC 1320 vector.
const MD4_EMPTY: &str = "31d6cfe0d16ae931b73c59d7e0c089c0";

// Byte i is i % 251. The period is built once and copied in whole periods, so the test
// does not loop per byte in the unoptimised test build.
fn pattern(size: usize) -> Vec<u8> {
    let period: Vec<u8> = (0..251).collect();
    let mut out = Vec::with_capacity(size + period.len());
    while out.len() < size {
        out.extend_from_slice(&period);
    }
    out.truncate(size);
    out
}

fn ed2k_hex(data: &[u8]) -> String {
    hash_reader(data, Select::default())
        .expect("hash a slice")
        .ed2k
}

// MD4 of each BLOCK-sized piece, computed with the md4 crate.
fn block_digests_hex(data: &[u8]) -> Vec<String> {
    data.chunks(BLOCK)
        .map(|block| hex::encode(Md4::digest(block)))
        .collect()
}

// The ed2k outer hash, built from block digests the test computed itself.
fn outer_hex(digests_hex: &[String]) -> String {
    let mut outer = Md4::new();
    for digest in digests_hex {
        outer.update(hex::decode(digest).expect("hex digest"));
    }
    hex::encode(outer.finalize())
}

// Plain MD4 of the data. Inputs: empty, 1 byte, and 9_727_999 bytes, all below one
// block. Expected values are OpenSSL output (see the header). "abc" is the RFC 1320
// vector.
#[test]
fn single_block_is_plain_md4() {
    assert_eq!(ed2k_hex(b""), MD4_EMPTY);
    assert_eq!(ed2k_hex(b"abc"), "a448017aaf21d8525fc10ae87aa6729d");
    assert_eq!(ed2k_hex(&pattern(1)), "47c61a0fa8738ba77308a8a600f88e4b");
    assert_eq!(
        ed2k_hex(&pattern(BLOCK - 1)),
        "8bc46f92ea3ad078998cc7baf97c31c3"
    );
}

// Multi-block values. The block digests and the ed2k hash are OpenSSL output for the
// pattern input (see the header). The test also computes the block digests itself with
// the md4 crate and checks them against the pinned ones, then checks the outer hash.
#[test]
fn multi_block_matches_precomputed_values() {
    // 9_728_001 bytes: one full block, then 1 byte (0xf4).
    check_multi(
        BLOCK + 1,
        &[
            "91c008dff530be53d16bdf71ee9ba342",
            "f2afb54a094c0397e996397459e0d7bb",
        ],
        "07149b89efa248c03d7e2c5e734d2d88",
    );
    // 19_456_000 bytes: exactly two full blocks.
    check_multi(
        2 * BLOCK,
        &[
            "91c008dff530be53d16bdf71ee9ba342",
            "54f7576746e704fd5c2903ddb7e96acb",
        ],
        "90955b3afd7d14b68b672c584f88dd93",
    );
    // 19_456_001 bytes: two full blocks, then 1 byte (0xed).
    check_multi(
        2 * BLOCK + 1,
        &[
            "91c008dff530be53d16bdf71ee9ba342",
            "54f7576746e704fd5c2903ddb7e96acb",
            "0c2c2674d1bce4453fae4ffdb6a234bb",
        ],
        "2bd1f2c5e81ab3f0fe4410ffe9658859",
    );
}

fn check_multi(size: usize, blocks: &[&str], ed2k: &str) {
    let data = pattern(size);
    let digests = block_digests_hex(&data);
    assert_eq!(digests, blocks, "block digests for {size} bytes");
    assert_eq!(outer_hex(&digests), ed2k, "outer hash for {size} bytes");
    assert_eq!(ed2k_hex(&data), ed2k, "crate ed2k for {size} bytes");
}

// Exact multiples of the block size. The crate gives no trailing empty block: a file of
// exactly one block hashes to that block's MD4, and a file of exactly two blocks hashes
// to the MD4 of its two block digests: a trailing block is only added when it holds data. The
// negative checks below are the alternative that appends the MD4 of empty input as a
// further block digest. They are OpenSSL output and must not match.
#[test]
fn exact_block_multiples_have_no_trailing_empty_block() {
    assert_eq!(ed2k::BLOCK_SIZE, BLOCK);

    let data = pattern(2 * BLOCK);
    // Exactly one block: MD4 of the block, 91c008df... (same as the block digest above).
    assert_eq!(ed2k_hex(&data[..BLOCK]), "91c008dff530be53d16bdf71ee9ba342");
    // Negative check: MD4(block digest || MD4("")) = 22155255...
    assert_ne!(ed2k_hex(&data[..BLOCK]), "22155255a2ed92712ccd01ad0eb9e8cb");
    // Exactly two blocks: MD4 of the two block digests, nothing appended.
    assert_eq!(ed2k_hex(&data), "90955b3afd7d14b68b672c584f88dd93");
    // Negative check: MD4(d1 || d2 || MD4("")) = b22937d7...
    assert_ne!(ed2k_hex(&data), "b22937d7a5bb74050fb54037fe573649");
}

// Fixed inputs, nothing computed. The name has spaces and non-ASCII characters. The link
// keeps the name as given, with no escaping.
#[test]
fn ed2k_link_is_exact() {
    let link = ed2k_link(
        "Frieren - 01 Ünïcødé 葬送.mkv",
        1_234_567_890,
        "0123456789abcdef0123456789abcdef",
    );
    assert_eq!(
        link,
        "ed2k://|file|Frieren - 01 Ünïcødé 葬送.mkv|1234567890|0123456789abcdef0123456789abcdef|/"
    );
}

// A reader whose reads end at the given offsets, in ascending order. The reads are still
// capped by the caller's buffer (1 MiB in hash_reader). `ends` records where each read
// stopped, so the test can check that a read stopped on a block boundary.
struct CutReader<'a> {
    data: &'a [u8],
    pos: usize,
    cuts: &'a [usize],
    ends: Vec<usize>,
}

impl Read for CutReader<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let next = self
            .cuts
            .iter()
            .copied()
            .find(|&cut| cut > self.pos)
            .unwrap_or(self.data.len());
        let end = next.min(self.data.len());
        let n = out.len().min(end - self.pos);
        out[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        self.ends.push(self.pos);
        Ok(n)
    }
}

// Reads of odd sizes must give the same hash as one read. The cut list has running sums
// of the sizes 1, 4_093, 65_537 and 999_983 (cycled), plus the block boundaries and one
// byte either side of each.
#[test]
fn odd_chunk_sizes_match_one_read() {
    let data = pattern(2 * BLOCK + 1);

    let mut cuts = Vec::new();
    let mut at = 0;
    for step in [1, 4_093, 65_537, 999_983].into_iter().cycle() {
        at += step;
        if at >= data.len() {
            break;
        }
        cuts.push(at);
    }
    cuts.extend([BLOCK - 1, BLOCK, BLOCK + 1, 2 * BLOCK, 2 * BLOCK + 1]);
    cuts.sort_unstable();
    cuts.dedup();

    let mut reader = CutReader {
        data: &data[..],
        pos: 0,
        cuts: &cuts,
        ends: Vec::new(),
    };
    let chunked = hash_reader(&mut reader, Select::default())
        .expect("hash chunked reads")
        .ed2k;
    assert!(
        reader.ends.contains(&BLOCK),
        "a read must end on the block boundary"
    );
    // Expected value: OpenSSL output for 19_456_001 bytes of the pattern (see above).
    assert_eq!(chunked, "2bd1f2c5e81ab3f0fe4410ffe9658859");
    assert_eq!(chunked, ed2k_hex(&data));
}
