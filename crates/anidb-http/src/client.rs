//! `AniDB` HTTP API client over a small, swappable transport.
//!
//! [`Transport`] is GET only, sends no `User-Agent` and caps a body at 128 MiB.
//! The `akiba-core` HTTP client has its own trait (GET and JSON POST, a
//! `User-Agent`, a smaller cap), because this crate cannot depend on `akiba-core`.

use std::fmt::Write as _;
use std::time::Duration;

use crate::tls::{automatic_agent, build_agent, is_tls_failure, TlsError, TlsOptions, CERT_HINT};
use crate::types::AniDbShow;
use crate::types::Error;
use crate::xml::parse_anime;

/// Default `AniDB` HTTP API endpoint.
pub const DEFAULT_BASE_URL: &str = "http://api.anidb.net:9001/httpapi";

const PROTOVER: u32 = 1;
const MAX_BODY: u64 = 128 * 1024 * 1024;

/// Transport level failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TransportError {
    #[error("HTTP status {0}")]
    Status(u16),
    #[error("{0}")]
    Io(String),
    #[error("{0} ({CERT_HINT})")]
    Tls(String),
}

/// Minimal blocking GET, so tests can use fakes instead of the network.
pub trait Transport {
    /// Fetches `url` with the given headers and returns the (decoded) body.
    ///
    /// # Errors
    ///
    /// Returns a [`TransportError`] when the request fails: a non-success HTTP status, an I/O failure or a TLS failure.
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, TransportError>;
}

impl<T: Transport + ?Sized> Transport for &T {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, TransportError> {
        (**self).get(url, headers)
    }
}

/// `ureq` backed transport (gzip content-encoding is decoded transparently).
#[derive(Debug, Clone)]
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    /// A transport that trusts what `tls` says. A bad CA file is an error here.
    ///
    /// # Errors
    ///
    /// Returns a [`TlsError`] when the CA file of `tls` cannot be read or holds no certificate.
    pub fn new(timeout: Duration, tls: &TlsOptions) -> Result<Self, TlsError> {
        Ok(Self {
            agent: build_agent(timeout, None, tls)?,
        })
    }
}

/// The system trust store and a 10 s timeout.
impl Default for UreqTransport {
    fn default() -> Self {
        Self {
            agent: automatic_agent(Duration::from_secs(10), None),
        }
    }
}

impl Transport for UreqTransport {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, TransportError> {
        let mut req = self.agent.get(url);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.call().map_err(|e| match e {
            ureq::Error::StatusCode(c) => TransportError::Status(c),
            other if is_tls_failure(&other) => TransportError::Tls(other.to_string()),
            other => TransportError::Io(other.to_string()),
        })?;
        resp.into_body()
            .with_config()
            .limit(MAX_BODY)
            .read_to_vec()
            .map_err(|e| TransportError::Io(e.to_string()))
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Detects an `AniDB` error reply in the body.
fn detect_error(body: &str) -> Option<Error> {
    const KNOWN: [(u16, &str); 5] = [
        (
            302,
            r#"<error code="302">client version missing or invalid</error>"#,
        ),
        (
            310,
            r#"<error code="310">illegal input or access denied</error>"#,
        ),
        (500, r#"<error code="500">banned</error>"#),
        (404, "<error>Anime not found</error>"),
        (998, "<error>aid Missing or Invalid</error>"),
    ];
    let code = KNOWN
        .iter()
        .find(|(_, s)| body.contains(s))
        .map(|(c, _)| *c)
        .or_else(|| {
            // generic `<error code="N">` fallback
            let rest = &body[body.find(r#"<error code=""#)? + r#"<error code=""#.len()..];
            rest[..rest.find('"')?].parse().ok()
        })?;
    Some(match code {
        302 => Error::ClientInvalid,
        500 => Error::Banned,
        404 | 998 => Error::AnimeNotFound,
        c => Error::Other(c),
    })
}

/// `AniDB` HTTP API client.
pub struct HttpApi<T: Transport = UreqTransport> {
    base_url: String,
    client: String,
    clientver: String,
    user_agent: String,
    transport: T,
}

impl HttpApi<UreqTransport> {
    /// A client for the HTTP API that trusts what `tls` says.
    ///
    /// # Errors
    ///
    /// Returns a [`TlsError`] when the CA file of `tls` cannot be read or holds no certificate.
    pub fn new(
        base_url: &str,
        client: &str,
        clientver: &str,
        user_agent: &str,
        tls: &TlsOptions,
    ) -> Result<Self, TlsError> {
        Ok(Self::with_transport(
            base_url,
            client,
            clientver,
            user_agent,
            UreqTransport::new(Duration::from_secs(10), tls)?,
        ))
    }
}

impl<T: Transport> HttpApi<T> {
    #[must_use]
    pub fn with_transport(
        base_url: &str,
        client: &str,
        clientver: &str,
        user_agent: &str,
        transport: T,
    ) -> Self {
        Self {
            base_url: base_url.to_owned(),
            client: client.to_owned(),
            clientver: clientver.to_owned(),
            user_agent: user_agent.to_owned(),
            transport,
        }
    }

    fn url(&self, aid: u32) -> String {
        format!(
            "{}?client={}&clientver={}&protover={PROTOVER}&request=anime&aid={aid}",
            self.base_url,
            percent_encode(&self.client),
            percent_encode(&self.clientver),
        )
    }

    /// Fetches and maps one anime.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] when the request fails ([`Error::Transport`]), the reply is an `<error>` element (for example [`Error::AnimeNotFound`] or [`Error::Banned`]), or the XML cannot be mapped ([`Error::Xml`]).
    pub fn anime(&self, aid: u32) -> Result<AniDbShow, Error> {
        let body = self.transport.get(
            &self.url(aid),
            &[
                ("User-Agent", self.user_agent.as_str()),
                ("Accept-Encoding", "gzip"),
            ],
        )?;
        let text = String::from_utf8_lossy(&body);
        if let Some(e) = detect_error(&text) {
            return Err(e);
        }
        parse_anime(&body)
    }
}
