//! Trace lines: one verbose line per event, off by default.
//!
//! Trace lines are separate from the warnings of [`crate::transport::Logger`].
//! Nothing secret is ever put in a line: parameters with a secret name are
//! replaced by `***` ([`redact_params`]), so are the session key and the salt
//! that replies carry ([`describe_reply`]), and ciphertext is only ever counted.
//! Every `format!` that feeds a trace line goes through these functions or
//! formats public values (command names, tags, codes, lengths, times).

use std::time::Instant;

use crate::error::Error;
use crate::transport::Logger;
use crate::wire::{escape_value, parse, Request};

/// The marker that replaces a secret.
pub(crate) const MASK: &str = "***";

/// Longest reply message kept in a line.
const MESSAGE_LIMIT: usize = 120;

/// The callback of the trace lines and the time they count from.
#[derive(Clone)]
pub(crate) struct Trace {
    sink: Logger,
    start: Instant,
}

impl Trace {
    pub(crate) fn new(sink: Logger, start: Instant) -> Self {
        Self { sink, start }
    }

    /// Passes one line, prefixed with the milliseconds since `start`.
    pub(crate) fn emit(&self, message: &str) {
        (self.sink)(&format!(
            "[{:>7} ms] {message}",
            self.start.elapsed().as_millis()
        ));
    }
}

/// Whether the value of a parameter called `name` is a secret or personal: the
/// password, the session key (`s`), the user name, and anything named like a
/// key, salt, secret or token.
fn is_secret_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "s"
        || name == "user"
        || ["pass", "key", "salt", "secret", "token", "sess"]
            .iter()
            .any(|part| name.contains(part))
}

/// The parameters of a request as they go on the wire (tag and session
/// included), with the secret values replaced by [`MASK`].
pub(crate) fn redact_params(request: &Request, has_session: bool, tag: &str) -> String {
    let trailer = std::iter::once(("tag", tag)).chain(has_session.then_some(("s", "")));
    request
        .params
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .chain(trailer)
        .map(|(key, value)| {
            if is_secret_name(key) {
                format!("{key}={MASK}")
            } else {
                format!("{key}={}", escape_value(value))
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// A reply message safe to print: the first word of a 200, 201 (session key)
/// and 209 (salt) is replaced, and for 200 and 201 so is the second word, the
/// `ip:port` the server sees (the user's address). The value of any `name=value`
/// word with a secret name is replaced too, and the rest is cut and escaped to
/// stay on one line.
fn redact_message(code: u16, message: &str) -> String {
    let secret_first_word = matches!(code, 200 | 201 | 209);
    let secret_address = matches!(code, 200 | 201);
    let words: Vec<String> = message
        .split_whitespace()
        .enumerate()
        .map(|(index, word)| match word.split_once('=') {
            _ if index == 0 && secret_first_word => MASK.to_owned(),
            _ if index == 1 && secret_address => MASK.to_owned(),
            Some((name, _)) if is_secret_name(name) => format!("{name}={MASK}"),
            _ => word.to_owned(),
        })
        .collect();
    let text: String = words.join(" ").chars().take(MESSAGE_LIMIT).collect();
    format!("{text:?}")
}

/// `code=.. tag=.. message=".." data_lines=..` of a reply text, or a note that
/// it is not a reply. Data lines are only counted, never printed.
pub(crate) fn describe_reply(text: &str) -> String {
    match parse(text) {
        Ok(reply) => format!(
            "code={} tag={} message={} data_lines={}",
            reply.code.code(),
            reply.tag.as_deref().unwrap_or("-"),
            redact_message(reply.code.code(), &reply.message),
            reply.lines.len()
        ),
        Err(_) => "not a parsable reply".to_owned(),
    }
}

/// `kind=.. code=..` of an error, never its text, which may quote a reply.
pub(crate) fn describe_error(error: &Error) -> String {
    format!(
        "kind={:?} code={}",
        error.kind(),
        error
            .code()
            .map_or_else(|| "-".to_owned(), |code| code.code().to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_parameters_are_masked_and_the_rest_is_kept() {
        let request = Request::new("AUTH")
            .param("user", "me")
            .param("pass", "hunter2")
            .param("apikey", "k1")
            .param("Salt", "x1")
            .param("x_token", "t1")
            .param("size", "5");
        let line = redact_params(&request, true, "ABCDE");
        assert_eq!(
            line,
            "user=***&pass=***&apikey=***&Salt=***&x_token=***&size=5&tag=ABCDE&s=***"
        );
        assert!(redact_params(&request, false, "ABCDE").ends_with("&tag=ABCDE"));
    }

    #[test]
    fn session_key_and_salt_of_replies_are_masked() {
        let auth = describe_reply("ABCDE 200 sEsSiOn 1.2.3.4:5 LOGIN ACCEPTED");
        assert_eq!(
            auth,
            "code=200 tag=ABCDE message=\"*** *** LOGIN ACCEPTED\" data_lines=0"
        );
        assert!(!auth.contains("1.2.3.4"));
        assert!(!describe_reply("ABCDE 209 saltsalt ENCRYPTION ENABLED").contains("saltsalt"));
        assert!(!describe_reply("ABCDE 201 sEsSiOn 1.2.3.4:5 NEW VERSION").contains("sEsSiOn"));
        assert!(!describe_reply("ABCDE 598 UNKNOWN pass=hunter2 s=abc").contains("hunter2"));
        assert!(!describe_reply("ABCDE 598 UNKNOWN pass=hunter2 s=abc").contains("abc"));
    }

    #[test]
    fn data_lines_are_counted_not_printed() {
        let line = describe_reply("ABCDE 220 FILE\nsecret|data\nmore");
        assert!(line.ends_with("data_lines=2"));
        assert!(!line.contains("secret"));
        assert_eq!(describe_reply("garbage"), "not a parsable reply");
    }
}
