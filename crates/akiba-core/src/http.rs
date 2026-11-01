//! Blocking HTTP behind a small trait, so that callers and tests can use a fake
//! instead of the network. The ID mapping providers and the anime list cache go
//! through it.
//!
//! It is separate from `anidb_http::Transport` (GET only, no `User-Agent`, a
//! 128 MiB cap), which the standalone `anidb-http` crate defines for itself:
//! this trait also posts JSON, sends `User-Agent: akiba/<VERSION>` and caps a body at 32 MiB.

use std::fmt::Write as _;
use std::time::Duration;

use anidb_http::tls::{build_agent, is_tls_failure, CERT_HINT};
use anidb_http::{TlsError, TlsOptions};
use serde_json::Value;

use crate::VERSION;

/// Upper bound on a response body. The anime list is the largest reply.
const MAX_BODY: u64 = 32 * 1024 * 1024;

/// Timeout of the downloads: id list, titles dump, HTTP API and poster.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Sends and receives HTTP. A non-success status is an error.
pub trait HttpClient {
    /// The body of a `GET` request.
    fn get_bytes(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, HttpError>;

    /// The body of a `GET` request, parsed as JSON.
    fn get_json(&self, url: &str, headers: &[(&str, &str)]) -> Result<Value, HttpError> {
        let body = self.get_bytes(url, headers)?;
        parse_json(url, &body)
    }

    /// Sends `body` as a JSON `POST` request and parses the JSON reply.
    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value, HttpError>;
}

/// A request that failed, with the URL and a description of why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot fetch {url}: {message}")]
pub struct HttpError {
    pub url: String,
    pub message: String,
}

impl HttpError {
    pub fn new(url: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            message: message.into(),
        }
    }
}

/// Parses a reply body as JSON. A bad body is an [`HttpError`] for `url`.
pub fn parse_json(url: &str, body: &[u8]) -> Result<Value, HttpError> {
    serde_json::from_slice(body).map_err(|e| HttpError::new(url, format!("invalid JSON: {e}")))
}

/// `base` with `params` appended as a query string. Names and values are
/// percent-encoded. `base` must not already have a query string.
#[must_use]
pub fn with_query(base: &str, params: &[(&str, &str)]) -> String {
    let mut url = base.to_owned();
    for (i, (name, value)) in params.iter().enumerate() {
        url.push(if i == 0 { '?' } else { '&' });
        url.push_str(&percent_encode(name));
        url.push('=');
        url.push_str(&percent_encode(value));
    }
    url
}

/// Percent-encodes everything except the RFC 3986 unreserved characters.
#[must_use]
pub fn percent_encode(s: &str) -> String {
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

/// `HttpClient` over `ureq`. The agent comes from [`anidb_http::tls`], so it
/// uses the platform TLS library and trust store (or the configured CA file), as
/// the portability rules require, and sends `User-Agent: akiba/<version>`.
#[derive(Debug, Clone)]
pub struct UreqClient {
    agent: ureq::Agent,
}

impl UreqClient {
    /// A bad CA file in `tls` is an error here, not at request time.
    pub fn new(timeout: Duration, tls: &TlsOptions) -> Result<Self, TlsError> {
        Ok(Self {
            agent: build_agent(timeout, Some(&format!("akiba/{VERSION}")), tls)?,
        })
    }
}

fn transport_error(url: &str, err: &ureq::Error) -> HttpError {
    let message = match err {
        ureq::Error::StatusCode(code) => format!("HTTP status {code}"),
        other if is_tls_failure(other) => format!("{other} ({CERT_HINT})"),
        other => other.to_string(),
    };
    HttpError::new(url, message)
}

impl HttpClient for UreqClient {
    fn get_bytes(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, HttpError> {
        let mut request = self.agent.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.call().map_err(|e| transport_error(url, &e))?;
        response
            .into_body()
            .with_config()
            .limit(MAX_BODY)
            .read_to_vec()
            .map_err(|e| HttpError::new(url, e.to_string()))
    }

    fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value, HttpError> {
        let mut request = self.agent.post(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request
            .send_json(body)
            .map_err(|e| transport_error(url, &e))?;
        response
            .into_body()
            .with_config()
            .limit(MAX_BODY)
            .read_json::<Value>()
            .map_err(|e| HttpError::new(url, format!("invalid JSON: {e}")))
    }
}

/// Canned replies for tests. Nothing here touches the network.
#[cfg(test)]
pub(crate) mod fake {
    use std::cell::RefCell;

    use super::{parse_json, HttpClient, HttpError};
    use serde_json::Value;

    /// One request that reached the fake.
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct Call {
        pub(crate) method: &'static str,
        pub(crate) url: String,
        pub(crate) headers: Vec<(String, String)>,
        pub(crate) body: Option<Value>,
    }

    impl Call {
        /// The value of the first header called `name`, compared without case.
        pub(crate) fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }
    }

    type Handler = Box<dyn Fn(&Call) -> Result<Vec<u8>, HttpError>>;

    /// Answers every request with the handler's reply and records the request.
    /// The handler sees the method, URL, headers and JSON body, so one route
    /// can serve several GraphQL queries. A request the handler rejects is an
    /// error, so an unexpected call fails the test.
    pub(crate) struct FakeHttp {
        handler: Handler,
        calls: RefCell<Vec<Call>>,
    }

    impl FakeHttp {
        pub(crate) fn new(handler: impl Fn(&Call) -> Result<Vec<u8>, HttpError> + 'static) -> Self {
            Self {
                handler: Box::new(handler),
                calls: RefCell::new(Vec::new()),
            }
        }

        /// A fake that refuses every request. Use it to check that no network
        /// call happens.
        pub(crate) fn offline() -> Self {
            Self::new(|call| Err(HttpError::new(&call.url, "network access is not allowed")))
        }

        /// Every request so far, in order.
        pub(crate) fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }

        fn reply(&self, call: Call) -> Result<Vec<u8>, HttpError> {
            let reply = (self.handler)(&call);
            self.calls.borrow_mut().push(call);
            reply
        }
    }

    /// The reply body for a JSON value. Always `Ok`, so that it can be used as a
    /// handler reply.
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn json(value: &Value) -> Result<Vec<u8>, HttpError> {
        Ok(serde_json::to_vec(value).expect("JSON value serialises"))
    }

    fn owned(headers: &[(&str, &str)]) -> Vec<(String, String)> {
        headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    impl HttpClient for FakeHttp {
        fn get_bytes(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, HttpError> {
            self.reply(Call {
                method: "GET",
                url: url.to_owned(),
                headers: owned(headers),
                body: None,
            })
        }

        fn post_json(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            body: &Value,
        ) -> Result<Value, HttpError> {
            let bytes = self.reply(Call {
                method: "POST",
                url: url.to_owned(),
                headers: owned(headers),
                body: Some(body.clone()),
            })?;
            parse_json(url, &bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_values_are_percent_encoded() {
        assert_eq!(
            with_query(
                "https://example.test/search",
                &[
                    ("api_key", "abc"),
                    ("query", "Kimi & Me [OVA]."),
                    ("include_adult", "true"),
                ],
            ),
            "https://example.test/search?api_key=abc&query=Kimi%20%26%20Me%20%5BOVA%5D.&include_adult=true"
        );
    }

    #[test]
    fn percent_encoding_keeps_unreserved_characters() {
        assert_eq!(percent_encode("a-b_c.d~E9"), "a-b_c.d~E9");
        assert_eq!(percent_encode("/?"), "%2F%3F");
        assert_eq!(percent_encode("é"), "%C3%A9");
    }

    #[test]
    fn bad_json_is_an_http_error_for_its_url() {
        let err = parse_json("https://example.test/x", b"[").expect_err("bad JSON");
        assert_eq!(err.url, "https://example.test/x");
        assert!(err.message.starts_with("invalid JSON"), "{err}");
    }

    #[test]
    fn the_production_client_builds_in_automatic_mode() {
        UreqClient::new(DEFAULT_TIMEOUT, &TlsOptions::automatic()).expect("client");
    }

    #[test]
    fn a_missing_ca_file_is_an_error_when_the_client_is_built() {
        let tls = TlsOptions::new(Some("/nonexistent/akiba/ca.pem".into()));
        let err = UreqClient::new(DEFAULT_TIMEOUT, &tls).expect_err("missing CA file");
        assert!(
            err.to_string().contains("/nonexistent/akiba/ca.pem"),
            "{err}"
        );
    }
}
