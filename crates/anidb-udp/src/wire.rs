//! Request encoding and reply parsing of the `AniDB` UDP API.
//!
//! A request is one datagram with one line, `COMMAND key=value&key=value`. A
//! reply is `{tag} {code} {text}` followed by data lines, each field separated
//! by `|`. Tags are only present on replies to tagged requests; errors that the
//! server sends without one (504, 555, 6xx) have no tag.
//!
//! This module implements the request and reply format of the specification,
//! see the crate root for the link. The escaping of values is the content
//! encoding rule of the spec.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use crate::codes::ResponseCode;
use crate::error::Error;

/// Characters a reply's tag is made of, see [`make_tag`].
const TAG_ALPHABET: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Length of a tag. Tags start with a letter or digit, never two zero bytes,
/// which mark compressed datagrams.
pub const TAG_LENGTH: usize = 5;

/// A request to the API: a command and its parameters, in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Request {
    /// The command name, such as `AUTH` or `FILE`.
    pub command: String,
    /// Parameters in the order they are sent. Values are escaped on encoding.
    pub params: Vec<(String, String)>,
}

impl Request {
    /// A request with no parameters.
    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            params: Vec::new(),
        }
    }

    /// Adds a parameter. The value is escaped when the request is encoded.
    #[must_use]
    pub fn param(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.push((key.to_owned(), value.into()));
        self
    }

    /// Encodes the request for the wire. `tag` is always sent, and `session` as
    /// `s` after it when there is one.
    ///
    /// Values are escaped like an HTML form: `&` becomes `&amp;`, and each
    /// newline (`\r\n`, `\r` or `\n`) becomes `<br />`.
    #[must_use]
    pub fn encode(&self, session: Option<&str>, tag: &str) -> String {
        let trailer = std::iter::once(("tag", tag)).chain(session.map(|s| ("s", s)));
        let params: Vec<String> = self
            .params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .chain(trailer)
            .map(|(key, value)| format!("{key}={}", escape_value(value)))
            .collect();
        format!("{} {}", self.command, params.join("&"))
    }
}

/// Escapes a request value like an HTML form: `&` becomes `&amp;`, and each
/// newline becomes `<br />`. `<br />` has no `&`, so the order of the two
/// rules does not matter.
pub(crate) fn escape_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                escaped.push_str("<br />");
            }
            '\n' => escaped.push_str("<br />"),
            '&' => escaped.push_str("&amp;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Creates a random tag of [`TAG_LENGTH`] letters and digits.
///
/// Tags only tell replies apart, so they need no cryptographic randomness. Each
/// [`RandomState`] is seeded from the operating system, so this needs no crate.
#[must_use]
pub fn make_tag() -> String {
    (0..TAG_LENGTH)
        .map(|_| {
            let hash = RandomState::new().build_hasher().finish();
            char::from(TAG_ALPHABET[usize::from(hash.to_le_bytes()[0]) % TAG_ALPHABET.len()])
        })
        .collect()
}

/// A parsed reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The tag of the request this reply answers, `None` for untagged replies.
    pub tag: Option<String>,
    /// The reply code. Codes the table does not list are
    /// [`ResponseCode::Unknown`], with the number kept.
    pub code: ResponseCode,
    /// The rest of the first line after the code, trimmed. Some replies carry
    /// data here, such as the session key of 200. These are 200, 201, 209, 271,
    /// 272 and 504.
    pub message: String,
    /// The data lines, each split into its `|` separated fields. Empty when the
    /// reply has no data lines.
    pub lines: Vec<Vec<String>>,
}

/// Parses one datagram (already decrypted and decompressed) as a reply.
///
/// The text is taken as UTF-8 by the caller. A reply that does not start with a
/// tag or a three digit code is an [`Error::BadResponse`]. Lines end with `\n`,
/// never with `\r\n`.
///
/// # Errors
///
/// Returns [`Error::BadResponse`] when `text` has no valid reply code.
pub fn parse(text: &str) -> Result<Response, Error> {
    let bytes = text.as_bytes();
    let (tag, body) = if split_code(text).is_some() {
        (None, text)
    } else if bytes.len() > TAG_LENGTH
        && bytes[..TAG_LENGTH].iter().all(u8::is_ascii_alphanumeric)
        && bytes[TAG_LENGTH] == b' '
    {
        (Some(&text[..TAG_LENGTH]), &text[TAG_LENGTH + 1..])
    } else {
        return Err(bad_response(text));
    };
    let Some((code, rest)) = split_code(body) else {
        return Err(bad_response(text));
    };

    let (first_line, data) = rest.split_once('\n').unwrap_or((rest, ""));
    let data = data.trim();
    let lines = if data.is_empty() {
        Vec::new()
    } else {
        data.split('\n')
            .map(|line| line.split('|').map(str::to_owned).collect())
            .collect()
    };
    Ok(Response {
        tag: tag.map(str::to_owned),
        code: ResponseCode::from(code),
        message: first_line.trim().to_owned(),
        lines,
    })
}

/// Splits `NNN rest` into the code and the rest of the text.
fn split_code(text: &str) -> Option<(u16, &str)> {
    let bytes = text.as_bytes();
    if bytes.len() < 4 || bytes[3] != b' ' || !bytes[..3].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let code = text[..3].parse().ok()?;
    Some((code, &text[4..]))
}

fn bad_response(text: &str) -> Error {
    // only the start is kept, a reply can be long and this only goes to a log
    Error::BadResponse(text.chars().take(64).collect())
}

#[cfg(test)]
mod tests;
