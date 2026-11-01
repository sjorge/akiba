use crate::client::TransportError;
use crate::tls::TlsError;

/// Errors from the HTTP API and the titles dump.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// `AniDB` error 302: client version missing or invalid.
    #[error("client version missing or invalid, register a HTTP client on anidb")]
    ClientInvalid,
    /// `AniDB` error 500: banned.
    #[error("banned, try again in 24h")]
    Banned,
    /// `AniDB` error 998 (or 404): anime id missing, invalid or not found.
    #[error("anime id missing, invalid or not found")]
    AnimeNotFound,
    /// Any other `AniDB` error code, e.g. 310.
    #[error("AniDB returned an error. Code {0}")]
    Other(u16),
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Tls(#[from] TlsError),
    #[error("invalid XML: {0}")]
    Xml(String),
    #[error("decompression failed: {0}")]
    Decompress(#[source] std::io::Error),
}
