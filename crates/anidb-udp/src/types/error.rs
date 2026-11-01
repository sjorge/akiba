//! Errors of the `AniDB` UDP client.

use crate::codes::ResponseCode;

/// Errors of the wire layer and of the client built on top of it.
///
/// Server replies are classified by [`Error::from_reply`], bans, server side errors and timeouts get their own
/// variants, everything else is a plain reply error.
///
/// Every reply code above 305 that a command did not expect is a failure. So
/// are the client side codes 670 to 673: unknown error, timeout, not connected
/// and failed to initialise. The client side codes are not in the spec, the
/// server never sends them. Some variants carry parsed data: the current
/// mylist entry for 310, the file ids for 322, and the vote record for 360,
/// 363 and 364.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A failure reply the command did not expect, such as 320 `NO_SUCH_FILE`.
    #[error("{message}")]
    Reply {
        /// The code the server sent.
        code: ResponseCode,
        /// The text of the server, or the code when the server sent none.
        message: String,
    },

    /// 504 or 555: the client or the IP is banned.
    #[error("banned: {message}")]
    Banned {
        /// The code the server sent.
        code: ResponseCode,
        /// The reason the server gave.
        message: String,
    },

    /// 6xx: the server failed, is busy or is out of service.
    #[error("server error: {message}")]
    Server {
        /// The code the server sent.
        code: ResponseCode,
        /// The text of the server, or the code when the server sent none.
        message: String,
    },

    /// No reply arrived in time (client side code 671).
    #[error("timed out: {message}")]
    Timeout {
        /// What timed out.
        message: String,
    },

    /// 310 `FILE_ALREADY_IN_MYLIST` on MYLISTADD without `edit`: carries the
    /// record that is already in the mylist.
    #[error("file already in mylist (pass edit to change it)")]
    FileAlreadyInMylist {
        /// The current mylist entry.
        entry: Box<crate::mylist::MylistEntry>,
    },

    /// 360, 363 or 364 on VOTE: the vote was refused, and the reply says for
    /// which entity.
    #[error("{message}")]
    VoteRefused {
        /// The code the server sent.
        code: ResponseCode,
        /// The text of the server.
        message: String,
        /// The vote record of the reply.
        vote: Box<crate::responses::VoteRecord>,
    },

    /// 322 `MULTIPLE_FILES_FOUND`: more than one file matches, these are their
    /// ids.
    #[error("multiple files found: {}", join_ids(fids))]
    MultipleFilesFound {
        /// The ids of the files found.
        fids: Vec<i64>,
    },

    /// The socket failed: resolving the host, binding, sending or receiving.
    #[error("socket error: {0}")]
    Io(#[from] std::io::Error),

    /// A datagram that is not a reply this client can read.
    #[error("unexpected reply: {0}")]
    BadResponse(String),

    /// The data of a reply does not match the fields its mask asked for: too
    /// few fields, or a value that does not parse.
    #[error("bad reply data: {0}")]
    BadData(String),

    /// An encrypted reply has a bad length or padding.
    #[error("cannot decrypt reply")]
    Decrypt,

    /// A compressed reply is corrupt.
    #[error(transparent)]
    Decompress(#[from] flate2::DecompressError),

    /// A request argument is not valid, checked before anything is sent.
    #[error("{0}")]
    InvalidArgument(String),
}

/// What an [`Error`] means for the rest of the session, see [`Error::kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Every later request on this client would fail too: a ban, an outage, a
    /// timeout, a lost socket or session, a rejected client, a reply that
    /// cannot be decrypted.
    SessionFatal,
    /// This request failed but the session is fine: a reply such as 320 or
    /// 321, a file that is already in the mylist, several files found, a
    /// refused vote, bad data in one reply.
    Request,
    /// The caller made a mistake and nothing was sent.
    Usage,
}

fn join_ids(ids: &[i64]) -> String {
    ids.iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

impl Error {
    /// Classifies a failure reply. An empty `message` is replaced by the code.
    #[must_use]
    pub fn from_reply(code: ResponseCode, message: &str) -> Self {
        let message = if message.is_empty() {
            code.to_string()
        } else {
            message.to_owned()
        };
        match code.code() {
            504 | 555 => Self::Banned { code, message },
            671 => Self::Timeout { message },
            // client side codes, the server never sends these
            670 | 672 | 673 => Self::Reply { code, message },
            600..=699 => Self::Server { code, message },
            _ => Self::Reply { code, message },
        }
    }

    /// The reply code behind the error: that of `Reply`, `Banned`, `Server` and
    /// `VoteRefused`, 671 for `Timeout`, 310 for `FileAlreadyInMylist`, 322 for
    /// `MultipleFilesFound`, `None` for the errors that are not a reply.
    #[must_use]
    pub fn code(&self) -> Option<ResponseCode> {
        match self {
            Self::Reply { code, .. }
            | Self::Banned { code, .. }
            | Self::Server { code, .. }
            | Self::VoteRefused { code, .. } => Some(*code),
            Self::Timeout { .. } => Some(ResponseCode::ConnectionTimeout),
            Self::FileAlreadyInMylist { .. } => Some(ResponseCode::FileAlreadyInMylist),
            Self::MultipleFilesFound { .. } => Some(ResponseCode::MultipleFilesFound),
            Self::Io(_)
            | Self::BadResponse(_)
            | Self::BadData(_)
            | Self::Decrypt
            | Self::Decompress(_)
            | Self::InvalidArgument(_) => None,
        }
    }

    /// The category of the error.
    ///
    /// - `SessionFatal`: `Banned`, `Server`, `Timeout`, `Io`, `BadResponse`,
    ///   `Decrypt`, and a `Reply` with a session, login or client code (403,
    ///   500 to 503, 505, 506, 509, 519, 598, 670, 672, 673 or a code this crate
    ///   does not know).
    /// - `Request`: every other `Reply` (such as 320), `FileAlreadyInMylist`,
    ///   `MultipleFilesFound`, `VoteRefused`, `BadData`, `Decompress`.
    /// - `Usage`: `InvalidArgument`.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Banned { .. }
            | Self::Server { .. }
            | Self::Timeout { .. }
            | Self::Io(_)
            | Self::BadResponse(_)
            | Self::Decrypt => ErrorKind::SessionFatal,
            Self::Reply { code, .. } => {
                if matches!(
                    code,
                    ResponseCode::NotLoggedIn
                        | ResponseCode::LoginFailed
                        | ResponseCode::LoginFirst
                        | ResponseCode::AccessDenied
                        | ResponseCode::ClientVersionOutdated
                        | ResponseCode::IllegalInputOrAccessDenied
                        | ResponseCode::InvalidSession
                        | ResponseCode::NoSuchEncryptionType
                        | ResponseCode::EncodingNotSupported
                        | ResponseCode::UnknownCommand
                        | ResponseCode::UnknownApiError
                        | ResponseCode::NotConnected
                        | ResponseCode::FailedToInitialize
                        | ResponseCode::Unknown(_)
                ) {
                    ErrorKind::SessionFatal
                } else {
                    ErrorKind::Request
                }
            }
            Self::FileAlreadyInMylist { .. }
            | Self::VoteRefused { .. }
            | Self::MultipleFilesFound { .. }
            | Self::BadData(_)
            | Self::Decompress(_) => ErrorKind::Request,
            Self::InvalidArgument(_) => ErrorKind::Usage,
        }
    }

    /// Whether every later request on this client would fail too: a ban, an
    /// outage, a timeout, a lost socket or session, a rejected client.
    ///
    /// This is the rule that stops an akiba rename run; every other error only
    /// fails the one file.
    #[must_use]
    pub fn is_session_fatal(&self) -> bool {
        self.kind() == ErrorKind::SessionFatal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_bans_timeouts_and_server_errors() {
        assert!(matches!(
            Error::from_reply(ResponseCode::Banned, "BANNED"),
            Error::Banned { .. }
        ));
        assert!(matches!(
            Error::from_reply(ResponseCode::ClientBanned, ""),
            Error::Banned { .. }
        ));
        assert!(matches!(
            Error::from_reply(ResponseCode::ConnectionTimeout, "no reply"),
            Error::Timeout { .. }
        ));
        assert!(matches!(
            Error::from_reply(ResponseCode::InternalServerError, ""),
            Error::Server { .. }
        ));
        assert!(matches!(
            Error::from_reply(ResponseCode::NoSuchFile, "NO SUCH FILE"),
            Error::Reply { .. }
        ));
        assert!(matches!(
            Error::from_reply(ResponseCode::NotConnected, ""),
            Error::Reply { .. }
        ));
    }

    #[test]
    fn empty_message_falls_back_to_the_code() {
        let error = Error::from_reply(ResponseCode::NoSuchFile, "");
        assert_eq!(error.to_string(), "320 NO_SUCH_FILE");
        let error = Error::from_reply(ResponseCode::NoSuchFile, "NO SUCH FILE");
        assert_eq!(error.to_string(), "NO SUCH FILE");
    }

    #[test]
    fn only_session_wide_errors_are_session_fatal() {
        let fatal = [
            Error::from_reply(ResponseCode::Banned, "BANNED"),
            Error::from_reply(ResponseCode::ClientBanned, ""),
            Error::from_reply(ResponseCode::AnidbOutOfService, "OUT"),
            Error::from_reply(ResponseCode::InternalServerError, ""),
            Error::Timeout {
                message: "no reply".to_owned(),
            },
            Error::Io(std::io::Error::other("no network")),
            Error::BadResponse("garbage".to_owned()),
            Error::Decrypt,
            Error::from_reply(ResponseCode::InvalidSession, "INVALID SESSION"),
            Error::from_reply(ResponseCode::LoginFailed, "LOGIN FAILED"),
        ];
        for error in &fatal {
            assert!(error.is_session_fatal(), "{error}");
        }

        let per_file = [
            Error::from_reply(ResponseCode::NoSuchFile, ""),
            Error::from_reply(ResponseCode::NoSuchEntry, ""),
            Error::from_reply(ResponseCode::NoSuchAnime, ""),
            Error::MultipleFilesFound { fids: vec![1, 2] },
            Error::BadData("bad size".to_owned()),
        ];
        for error in &per_file {
            assert!(!error.is_session_fatal(), "{error}");
        }
    }

    #[test]
    fn code_is_the_reply_code_behind_the_error() {
        let reply = Error::from_reply(ResponseCode::NoSuchFile, "");
        assert_eq!(reply.code(), Some(ResponseCode::NoSuchFile));
        let banned = Error::from_reply(ResponseCode::Banned, "");
        assert_eq!(banned.code(), Some(ResponseCode::Banned));
        let server = Error::from_reply(ResponseCode::AnidbOutOfService, "");
        assert_eq!(server.code(), Some(ResponseCode::AnidbOutOfService));
        let timeout = Error::Timeout {
            message: String::new(),
        };
        assert_eq!(timeout.code(), Some(ResponseCode::ConnectionTimeout));
        let multiple = Error::MultipleFilesFound { fids: vec![1] };
        assert_eq!(multiple.code(), Some(ResponseCode::MultipleFilesFound));
        assert_eq!(Error::Decrypt.code(), None);
        assert_eq!(Error::InvalidArgument("x".to_owned()).code(), None);
    }

    #[test]
    fn kind_is_pinned_for_every_variant() {
        use std::io;
        let entry = crate::mylist::MylistEntry {
            lid: 1,
            fid: 2,
            eid: 3,
            aid: 4,
            gid: 5,
            date: 0,
            state: None,
            viewdate: 0,
            storage: String::new(),
            source: String::new(),
            other: String::new(),
            filestate: None,
        };
        let vote = crate::responses::VoteRecord {
            name: String::new(),
            vote_value: 0,
            vote_type: None,
            id: 0,
        };
        let cases: Vec<(Error, ErrorKind)> = vec![
            (
                Error::from_reply(ResponseCode::NoSuchFile, ""),
                ErrorKind::Request,
            ),
            (
                Error::from_reply(ResponseCode::LoginFailed, ""),
                ErrorKind::SessionFatal,
            ),
            (
                Error::from_reply(ResponseCode::Unknown(999), ""),
                ErrorKind::SessionFatal,
            ),
            (
                Error::from_reply(ResponseCode::Banned, ""),
                ErrorKind::SessionFatal,
            ),
            (
                Error::from_reply(ResponseCode::InternalServerError, ""),
                ErrorKind::SessionFatal,
            ),
            (
                Error::Timeout {
                    message: String::new(),
                },
                ErrorKind::SessionFatal,
            ),
            (
                Error::FileAlreadyInMylist {
                    entry: Box::new(entry),
                },
                ErrorKind::Request,
            ),
            (
                Error::VoteRefused {
                    code: ResponseCode::NoSuchVote,
                    message: String::new(),
                    vote: Box::new(vote),
                },
                ErrorKind::Request,
            ),
            (
                Error::MultipleFilesFound { fids: vec![1] },
                ErrorKind::Request,
            ),
            (Error::Io(io::Error::other("x")), ErrorKind::SessionFatal),
            (Error::BadResponse(String::new()), ErrorKind::SessionFatal),
            (Error::BadData(String::new()), ErrorKind::Request),
            (Error::Decrypt, ErrorKind::SessionFatal),
            (
                // zlib header, then a block with the reserved type 3
                crate::compression::decompress(&[0, 0, 0x78, 0x9c, 0xff, 0xff, 0xff]).unwrap_err(),
                ErrorKind::Request,
            ),
            (Error::InvalidArgument(String::new()), ErrorKind::Usage),
        ];
        for (error, kind) in &cases {
            assert_eq!(error.kind(), *kind, "{error:?}");
        }
        // an exhaustive match, so a new variant has to be added to the cases above
        for (error, _) in &cases {
            match error {
                Error::Reply { .. }
                | Error::Banned { .. }
                | Error::Server { .. }
                | Error::Timeout { .. }
                | Error::FileAlreadyInMylist { .. }
                | Error::VoteRefused { .. }
                | Error::MultipleFilesFound { .. }
                | Error::Io(_)
                | Error::BadResponse(_)
                | Error::BadData(_)
                | Error::Decrypt
                | Error::Decompress(_)
                | Error::InvalidArgument(_) => {}
            }
        }
    }
}
