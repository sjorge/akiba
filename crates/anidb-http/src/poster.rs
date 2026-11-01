//! `AniDB` CDN posters. The anime API names a poster by its `picture` file name,
//! and the image lives at [`POSTER_BASE_URL`].

use crate::client::Transport;
use crate::types::Error;

/// Base URL of the `AniDB` CDN images that the `picture` field names.
pub const POSTER_BASE_URL: &str = "https://cdn.anidb.net/images/main/";

/// The URL of the poster named by `picture`, such as `12345.jpg`.
#[must_use]
pub fn poster_url(picture: &str) -> String {
    format!("{POSTER_BASE_URL}{picture}")
}

/// Downloads the poster named by `picture` over `transport` and returns the
/// bytes unchanged. `picture` should be non-empty; the caller decides whether to
/// fetch at all and where the bytes are stored.
///
/// The transport's own body limit applies. [`UreqTransport`](crate::UreqTransport)
/// allows 128 MiB, so this function cannot enforce a smaller cap.
///
/// # Errors
///
/// Returns [`Error::Transport`] when the download fails.
pub fn fetch_poster(transport: &dyn Transport, picture: &str) -> Result<Vec<u8>, Error> {
    Ok(transport.get(&poster_url(picture), &[])?)
}
