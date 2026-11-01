use super::*;

const CA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/test-ca.pem");
const T: Duration = Duration::from_secs(5);

/// A file in a fresh temp dir, which lives as long as the returned guard.
fn temp(name: &str, content: &[u8]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(name);
    fs::write(&path, content).expect("write");
    (dir, path)
}

#[test]
fn automatic_mode_builds() {
    let options = TlsOptions::automatic();
    assert!(options.ca_file().is_none());
    build_agent(T, Some("akiba/test"), &options).expect("agent");
    let _ = automatic_agent(T, None);
}

#[test]
fn an_empty_path_is_automatic() {
    assert_eq!(
        TlsOptions::new(Some(PathBuf::new())),
        TlsOptions::automatic()
    );
    assert_eq!(TlsOptions::new(None), TlsOptions::automatic());
}

#[test]
fn a_valid_pem_file_builds() {
    let options = TlsOptions::new(Some(PathBuf::from(CA)));
    options.validate().expect("valid");
    build_agent(T, None, &options).expect("agent");
}

#[test]
fn a_bundle_with_other_pem_items_builds() {
    let mut bundle = b"# a comment\n".to_vec();
    bundle.extend(fs::read(CA).expect("fixture"));
    bundle.extend(fs::read(CA).expect("fixture"));
    let (_dir, path) = temp("bundle.pem", &bundle);
    TlsOptions::new(Some(path)).validate().expect("valid");
}

#[test]
fn a_missing_file_is_an_error() {
    let options = TlsOptions::new(Some(PathBuf::from("/nonexistent/akiba/ca.pem")));
    let err = build_agent(T, None, &options).expect_err("missing");
    assert!(matches!(err, TlsError::Read { .. }), "{err}");
    assert!(err.to_string().contains("/nonexistent/akiba/ca.pem"));
}

#[test]
fn a_directory_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let options = TlsOptions::new(Some(dir.path().to_path_buf()));
    assert!(matches!(
        options.validate().expect_err("directory"),
        TlsError::Read { .. }
    ));
}

#[test]
fn an_empty_file_is_an_error() {
    let (_dir, path) = temp("empty.pem", b"");
    let options = TlsOptions::new(Some(path));
    assert!(matches!(
        options.validate().expect_err("empty"),
        TlsError::NoCertificates { .. }
    ));
}

#[test]
fn a_file_that_is_not_pem_is_an_error() {
    let (_dir, path) = temp("text.pem", b"hello, not a certificate\n");
    let options = TlsOptions::new(Some(path));
    assert!(options.validate().is_err());
}

#[test]
fn a_broken_pem_block_is_an_error() {
    let pem = b"-----BEGIN CERTIFICATE-----\n!!!! not base64 !!!!\n-----END CERTIFICATE-----\n";
    let (_dir, path) = temp("broken.pem", pem);
    let options = TlsOptions::new(Some(path));
    assert!(options.validate().is_err());
}

#[test]
fn a_certificate_block_that_is_not_a_certificate_is_an_error() {
    // valid base64 of the bytes "hello world"
    let pem = b"-----BEGIN CERTIFICATE-----\naGVsbG8gd29ybGQ=\n-----END CERTIFICATE-----\n";
    let (_dir, path) = temp("fake.pem", pem);
    let options = TlsOptions::new(Some(path));
    assert!(matches!(
        options.validate().expect_err("fake"),
        TlsError::InvalidPem { .. }
    ));
}

#[test]
fn a_pem_file_without_certificates_is_an_error() {
    let pem = b"-----BEGIN PRIVATE KEY-----\naGVsbG8gd29ybGQ=\n-----END PRIVATE KEY-----\n";
    let (_dir, path) = temp("key.pem", pem);
    let options = TlsOptions::new(Some(path));
    assert!(matches!(
        options.validate().expect_err("key only"),
        TlsError::NoCertificates { .. }
    ));
}

#[test]
fn the_hint_names_the_way_out() {
    assert!(CERT_HINT.contains("tls.ca_file"));
    assert!(CERT_HINT.contains("--tls-ca-file"));
}
