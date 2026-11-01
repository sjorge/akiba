//! Host identity. The hash cache is named after the sha256 of the hostname, so
//! there is no per-OS machine-id code.

use std::io;

use sha2::{Digest, Sha256};

/// The hostname, via the POSIX `gethostname` call.
pub fn hostname() -> io::Result<String> {
    gethostname::gethostname()
        .into_string()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "hostname is not valid UTF-8"))
}

/// Lower-case hex sha256 of `name`, the hash cache file stem.
pub fn hashed(name: &str) -> String {
    hex::encode(Sha256::digest(name.as_bytes()))
}

/// The fixed local UDP port for the `AniDB` API, derived from the hostname:
/// `10000 + (first 4 bytes of sha256(hostname), big endian) % 50000`. The spec
/// asks clients to keep one local port, and the same host name always gives the
/// same port, also for other tools using the same formula.
pub fn local_udp_port(name: &str) -> u16 {
    let digest: [u8; 32] = Sha256::digest(name.as_bytes()).into();
    let [a, b, c, d, ..] = digest;
    let head = u32::from_be_bytes([a, b, c, d]);
    // the remainder is below 50000, so the sum is below 60000 and fits
    10_000 + u16::try_from(head % 50_000).unwrap_or(0)
}

/// True for `localhost`, in any case. Such a name is not unique between hosts.
pub fn is_localhost(name: &str) -> bool {
    name.eq_ignore_ascii_case("localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_matches_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/host-id.json"
        )))
        .expect("host-id.json is valid JSON");
        let hostname = fixture["hostname"].as_str().expect("hostname");
        let expected = fixture["hashed"].as_str().expect("hashed");

        assert_eq!(hashed(hostname), expected);
        assert_eq!(
            hashed("test-host"),
            "c80cb283080a15df7c78360a7fd59c3f4e618732e33b206bb0343240c2cdf38b"
        );
    }

    #[test]
    fn hashed_is_lower_case_hex_of_full_digest() {
        let digest = hashed("");
        assert_eq!(digest.len(), 64);
        assert!(digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(
            digest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn local_udp_port_matches_independent_values() {
        // sha256 head and port computed with `openssl dgst -sha256` and shell
        // arithmetic: 10000 + 0xHEAD % 50000.
        for (name, port) in [
            ("test-host", 35331),       // head c80cb283
            ("localhost", 30725),       // head 49960de5
            ("", 22610),                // head e3b0c442
            ("nas.example.org", 55527), // head e273e2d7
            ("akiba", 33637),           // head ffb18395
        ] {
            assert_eq!(local_udp_port(name), port, "{name:?}");
        }
    }

    #[test]
    fn localhost_is_case_insensitive_and_exact() {
        assert!(is_localhost("localhost"));
        assert!(is_localhost("LocalHost"));
        assert!(!is_localhost("localhost.localdomain"));
        assert!(!is_localhost("some.long.hostname"));
    }

    #[test]
    fn hostname_is_non_empty() {
        assert_ne!(hostname().expect("hostname"), "");
    }
}
