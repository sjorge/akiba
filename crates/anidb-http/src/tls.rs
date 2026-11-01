//! TLS settings shared by every HTTP client in the workspace.
//!
//! The TLS library is the platform's (`native-tls`) and so is the trust store:
//! unless a CA file is given, the certificates the operating system trusts are
//! used. [`build_agent`] is the only place that builds a `ureq` agent.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ureq::tls::{parse_pem, Certificate, PemItem, RootCerts, TlsConfig, TlsProvider};

/// Appended to a TLS failure, so the user knows the way out.
pub const CERT_HINT: &str = "if the server certificate could not be verified by the system \
certificate store, set tls.ca_file (or run configure --tls-ca-file) to a CA bundle in PEM format";

/// Why the TLS settings cannot be used.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TlsError {
    #[error("cannot read the CA file {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the CA file {} is not valid PEM: {message}", path.display())]
    InvalidPem { path: PathBuf, message: String },
    #[error("the CA file {} contains no certificate", path.display())]
    NoCertificates { path: PathBuf },
}

/// Where the trusted root certificates come from.
///
/// - `ca_file` unset (the default): the system trust store, found
///   automatically. On macOS that is the Security framework, elsewhere it is
///   `OpenSSL`'s default locations, `SSL_CERT_FILE` / `SSL_CERT_DIR` and the
///   well known CA bundle paths of the OS.
/// - `ca_file` set: only the certificates in that PEM file are trusted, and
///   nothing is searched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TlsOptions {
    ca_file: Option<PathBuf>,
}

impl TlsOptions {
    /// The system trust store.
    #[must_use]
    pub fn automatic() -> Self {
        Self::default()
    }

    /// `ca_file` replaces the discovery. `None` and an empty path mean automatic.
    #[must_use]
    pub fn new(ca_file: Option<PathBuf>) -> Self {
        Self {
            ca_file: ca_file.filter(|p| !p.as_os_str().is_empty()),
        }
    }

    #[must_use]
    pub fn ca_file(&self) -> Option<&Path> {
        self.ca_file.as_deref()
    }

    /// Reads and checks the CA file, when there is one.
    ///
    /// # Errors
    ///
    /// Returns a [`TlsError`] when the CA file cannot be read or holds no certificate.
    pub fn validate(&self) -> Result<(), TlsError> {
        self.root_certs().map(drop)
    }

    fn root_certs(&self) -> Result<RootCerts, TlsError> {
        match &self.ca_file {
            None => Ok(RootCerts::PlatformVerifier),
            Some(path) => Ok(RootCerts::new_with_certs(&load_ca_file(path)?)),
        }
    }
}

/// The certificates in a PEM file. A file that cannot be read, is not PEM or
/// has no certificate is an error.
fn load_ca_file(path: &Path) -> Result<Vec<Certificate<'static>>, TlsError> {
    let bytes = fs::read(path).map_err(|source| TlsError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut certs = Vec::new();
    for item in parse_pem(&bytes) {
        let item = item.map_err(|e| TlsError::InvalidPem {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        if let PemItem::Certificate(cert) = item {
            // The TLS library skips a certificate it cannot parse, which would
            // leave nothing trusted without a word. A DER certificate is a
            // SEQUENCE, so anything else is rejected here.
            if cert.der().first() != Some(&0x30) {
                return Err(TlsError::InvalidPem {
                    path: path.to_path_buf(),
                    message: "a certificate is not DER encoded".to_owned(),
                });
            }
            certs.push(cert);
        }
    }
    if certs.is_empty() {
        return Err(TlsError::NoCertificates {
            path: path.to_path_buf(),
        });
    }
    Ok(certs)
}

/// An agent with the platform TLS and `root_certs`. Both are always set
/// explicitly: ureq's default roots are not compiled in, and asking for them
/// panics when the first request is made.
fn agent(timeout: Duration, user_agent: Option<&str>, root_certs: RootCerts) -> ureq::Agent {
    let mut builder = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(root_certs)
                .build(),
        );
    if let Some(user_agent) = user_agent {
        builder = builder.user_agent(user_agent);
    }
    builder.build().into()
}

/// The system trust store, which cannot fail to configure.
pub(crate) fn automatic_agent(timeout: Duration, user_agent: Option<&str>) -> ureq::Agent {
    agent(timeout, user_agent, RootCerts::PlatformVerifier)
}

/// Builds the one kind of agent the workspace uses. A bad CA file is an error
/// here, not a panic or a failure at request time.
///
/// # Errors
///
/// Returns a [`TlsError`] when the CA file of `tls` cannot be read or holds no certificate.
pub fn build_agent(
    timeout: Duration,
    user_agent: Option<&str>,
    tls: &TlsOptions,
) -> Result<ureq::Agent, TlsError> {
    Ok(agent(timeout, user_agent, tls.root_certs()?))
}

/// Whether `err` came from the TLS layer (handshake, certificate check).
#[must_use]
pub fn is_tls_failure(err: &ureq::Error) -> bool {
    matches!(err, ureq::Error::NativeTls(_) | ureq::Error::Tls(_))
}

#[cfg(test)]
mod tests;
