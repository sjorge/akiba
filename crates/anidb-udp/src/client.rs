//! Session client: AUTH/ENCRYPT/LOGOUT/PING and the typed commands akiba uses.
//!
//! The session commands (AUTH, ENCRYPT, LOGOUT) and PING are described in the
//! specification, see the crate root for the link. The session is kept until
//! LOGOUT, or until the server has seen no data for 35 minutes.
//!
//! Every command goes through [`Client::call`], which checks the reply code,
//! maps failures to [`Error`] and logs in again once when the server lost the
//! session. A new command is a [`Request`], the codes it accepts and a decoder
//! for the reply, see [`Client::file_by_hash`] for the shape.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::codes::ResponseCode;
use crate::constants::{
    HOST, PING_INTERVAL, PORT, PROTOCOL_VERSION, REQUEST_INTERVAL, REQUEST_TIMEOUT, UPTIME_INTERVAL,
};
use crate::crypto::Key;
use crate::error::Error;
use crate::file::{self, FileReply};
use crate::mask::{AnimeFields, FileFields};
use crate::mylist::{MylistEntry, MylistState};
use crate::trace::{describe_error, Trace};
use crate::transport::{Logger, Transport};
use crate::wire::{Request, Response};

/// Hint added to bans and outages.
const WAIT_HINT: &str = "Wait at least 30 minutes before trying again.";

/// Settings of a [`Client`].
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The registered client name, 4 to 16 lower case letters. Register it at
    /// <https://anidb.net/software/add>. The spec forbids the name of another
    /// client.
    pub client: String,
    /// The client version, a positive integer. It increases with every release
    /// of the client.
    pub clientver: u32,
    /// Host of the API, resolved once on connect.
    pub host: String,
    /// Port of the API.
    pub port: u16,
    /// Minimum time between two request starts. Do not go below the default
    /// against the real server.
    pub interval: Duration,
    /// Time to wait for a reply.
    pub timeout: Duration,
    /// Fixed local UDP port to bind. The spec asks for one, since too many
    /// different source ports from one IP get it banned. `None` lets the system
    /// pick a port for every connection.
    pub local_port: Option<u16>,
    /// Bind a system picked port, with a warning, when `local_port` is busy.
    /// When `false` a busy port is an error. Default `true`.
    pub local_port_fallback: bool,
    /// Idle time after which a PING is due, behind NAT. See
    /// [`Client::keep_alive_due`].
    pub ping_interval: Duration,
    /// Idle time after which an UPTIME is due, when not behind NAT.
    pub uptime_interval: Duration,
}

impl ClientConfig {
    /// A config for the real server with the default interval and timeout.
    #[must_use]
    pub fn new(client: impl Into<String>, clientver: u32) -> Self {
        Self {
            client: client.into(),
            clientver,
            host: HOST.to_owned(),
            port: PORT,
            interval: REQUEST_INTERVAL,
            timeout: REQUEST_TIMEOUT,
            local_port: None,
            local_port_fallback: true,
            ping_interval: PING_INTERVAL,
            uptime_interval: UPTIME_INTERVAL,
        }
    }
}

/// Options of MYLISTADD.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MylistAdd {
    /// Edit an entry that exists already, instead of failing with 310.
    pub edit: bool,
    /// The storage state to set.
    pub state: Option<MylistState>,
}

#[derive(Debug, Clone)]
struct Credentials {
    user: String,
    pass: String,
}

/// A blocking `AniDB` UDP client with one session.
///
/// Commands are sent one at a time, and the session is kept until
/// [`Client::disconnect`].
///
/// There is no hidden timer thread: the client is not `Send` and a one-shot
/// program has no idle time. A long-lived session is kept alive by polling:
/// every command first sends the PING or UPTIME that is due, and a program that
/// sits idle between commands calls [`Client::keep_alive`] from its own loop,
/// using [`Client::keep_alive_due`] to know when. Typical use: [`Client::connect`], which logs in, then
/// the commands, then [`Client::disconnect`].
///
/// The spec wants clients to cache what they have asked for. `AniDB` may ban
/// clients that ask for the same data again and again. This client does not
/// cache, that is up to the caller.
///
/// The rate limit belongs to one client. Several clients in one process, or
/// several processes, from one IP do not coordinate their requests, so together
/// they can exceed `AniDB`'s limits and get banned. Use one client, on a fixed
/// local port ([`ClientConfig::local_port`]).
pub struct Client {
    config: ClientConfig,
    transport: Option<Transport>,
    log: Option<Logger>,
    /// The trace callback, with the creation of the client as time zero.
    trace: Option<Logger>,
    created: Instant,
    credentials: Option<Credentials>,
    api_key: Option<String>,
    /// Set after a ban (504 or 555). Nothing is sent to `AniDB` again.
    banned: Option<(ResponseCode, String)>,
    /// Behind NAT, as far as known. Assumed until AUTH says otherwise.
    behind_nat: bool,
    /// When the last reply came in, the idle time for the keep-alive counts from it.
    last_request: Instant,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.config)
            .field("transport", &self.transport)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Creates a client. Nothing is sent until [`Client::connect`].
    #[must_use]
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            transport: None,
            log: None,
            trace: None,
            created: Instant::now(),
            credentials: None,
            api_key: None,
            banned: None,
            behind_nat: true,
            last_request: Instant::now(),
        }
    }

    /// Sets the callback that receives warnings. The library prints nothing
    /// itself, the callback decides what happens to them.
    pub fn set_logger(&mut self, log: impl Fn(&str) + Send + Sync + 'static) {
        let log: Logger = Arc::new(log);
        if let Some(transport) = &mut self.transport {
            transport.set_logger(Some(Arc::clone(&log)));
        }
        self.log = Some(log);
    }

    /// Sets the callback that receives trace lines: one verbose line per event
    /// (requests, replies, timeouts, key and session changes, keep-alive and
    /// rate-limiter waits), each prefixed with the milliseconds since the client
    /// was created. Off by default, and separate from [`Client::set_logger`].
    ///
    /// Passwords, API keys, salts, session keys and ciphertext never appear in a
    /// line: secret parameters and reply values show as `***`, and datagrams are
    /// only counted in bytes.
    pub fn set_trace(&mut self, trace: impl Fn(&str) + Send + Sync + 'static) {
        let trace: Logger = Arc::new(trace);
        if let Some(transport) = &mut self.transport {
            transport.set_trace(Some(Arc::clone(&trace)), self.created);
        }
        self.trace = Some(trace);
    }

    /// Passes a trace line, which is only built when tracing is on.
    fn trace(&self, line: impl FnOnce() -> String) {
        if let Some(sink) = &self.trace {
            Trace::new(Arc::clone(sink), self.created).emit(&line());
        }
    }

    fn warn(&self, message: &str) {
        if let Some(log) = &self.log {
            log(message);
        }
    }

    /// Whether a session is open.
    #[must_use]
    pub fn is_logged_in(&self) -> bool {
        self.transport
            .as_ref()
            .is_some_and(|transport| transport.session.is_some())
    }

    /// Opens the socket and logs in. With an `api_key` an ENCRYPT request goes
    /// first and every packet after it is encrypted. The api key is the UDP API
    /// key of the profile page, not the password.
    ///
    /// `ENCRYPT user=&type=1` replies 209 with a salt, and the AES key is the
    /// md5 hash of the api key followed by the salt. A normal AUTH is still
    /// needed after it.
    ///
    /// The previous connection is closed first. A connection that could not be
    /// set up is closed again.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn connect(&mut self, user: &str, pass: &str, api_key: Option<&str>) -> Result<(), Error> {
        if let Some((code, message)) = &self.banned {
            return Err(Error::from_reply(*code, message));
        }
        self.disconnect();
        if !is_valid_client_name(&self.config.client) {
            self.warn(&format!(
                "AniDB client name \"{}\" should be 4 to 16 lower case letters (a-z)",
                self.config.client
            ));
        }
        let mut transport = Transport::open(&self.config, self.log.clone())?;
        if let Some(trace) = &self.trace {
            transport.set_trace(Some(Arc::clone(trace)), self.created);
        }
        self.transport = Some(transport);
        self.api_key = api_key.filter(|key| !key.is_empty()).map(str::to_owned);

        self.trace(|| format!("login started, encrypted={}", self.api_key.is_some()));
        let result = self.setup(user, pass, "login");
        if let Err(error) = &result {
            self.trace(|| format!("login failed: {}", describe_error(error)));
        }
        if result.is_err() {
            self.close();
        }
        result
    }

    /// ENCRYPT first when there is an api key, so that the password never goes
    /// out in the clear, then AUTH. The first login and every re-login use this.
    fn setup(&mut self, user: &str, pass: &str, phase: &str) -> Result<(), Error> {
        // ENCRYPT needs no session, and a stale one must not be sent with it
        self.transport_mut()?.session = None;
        if let Some(api_key) = self.api_key.clone() {
            self.trace(|| format!("{phase} step ENCRYPT"));
            self.encrypt(user, &api_key)?;
        }
        self.trace(|| format!("{phase} step AUTH"));
        self.login(user, pass)
    }

    /// ENCRYPT, then every later packet is encrypted with the key it gives.
    fn encrypt(&mut self, user: &str, api_key: &str) -> Result<(), Error> {
        let response = self.call(
            &Request::new("ENCRYPT")
                .param("user", user)
                .param("type", "1"),
            &[ResponseCode::EncryptionEnabled],
        )?;
        let salt = first_word(&response.message).ok_or_else(|| {
            Error::BadResponse(format!("ENCRYPT reply has no salt: {}", response.message))
        })?;
        self.transport_mut()?.key = Some(Key::new(api_key, salt));
        self.trace(|| "encryption key set".to_owned());
        Ok(())
    }

    /// Logs in again after the server lost the session. A lost session also
    /// ends the encryption, so the key is dropped and the login is done as the
    /// first one was: ENCRYPT in the clear (it carries only the user name), then
    /// AUTH encrypted. Without an api key it is just an AUTH.
    fn relogin(&mut self, user: &str, pass: &str) -> Result<(), Error> {
        self.transport_mut()?.key = None;
        self.trace(|| "encryption key dropped".to_owned());
        self.setup(user, pass, "re-login")
    }

    /// AUTH `user=&pass=&protover=&client=&clientver=&nat=1&comp=1&enc=UTF8`.
    /// It replies 200 or 201 (a newer client version is available) with the
    /// session key, 500 LOGIN FAILED, 503 CLIENT VERSION OUTDATED, 504 CLIENT
    /// BANNED, 505 and 601.
    ///
    /// - The password is the normal website password.
    /// - The session key is 4 to 8 characters of a-z, A-Z and 0-9. It is sent as
    ///   `s` with every later command, and it is valid until LOGOUT or until the
    ///   server has seen no data for 35 minutes.
    /// - `nat=1`: the reply also carries `ip:port` as the server sees it. A port
    ///   other than the local one means NAT, see [`Client::behind_nat`].
    /// - `comp=1`: the server may deflate long replies instead of truncating them.
    /// - `enc=UTF8`: the server encoding is switched right away. Unsupported
    ///   encodings are silently ignored by AUTH.
    fn login(&mut self, user: &str, pass: &str) -> Result<(), Error> {
        let request = Request::new("AUTH")
            .param("user", user)
            .param("pass", pass)
            .param("protover", PROTOCOL_VERSION.to_string())
            .param("client", self.config.client.clone())
            .param("clientver", self.config.clientver.to_string())
            .param("nat", "1")
            .param("comp", "1")
            .param("enc", "UTF8");
        self.transport_mut()?.session = None;
        let response = self.call(
            &request,
            &[
                ResponseCode::LoginAccepted,
                ResponseCode::LoginAcceptedNewVersion,
            ],
        )?;
        let session = first_word(&response.message).ok_or_else(|| {
            Error::BadResponse(format!("AUTH reply has no session: {}", response.message))
        })?;
        if response.code == ResponseCode::LoginAcceptedNewVersion {
            self.warn("AniDB reports a newer client version is available");
        }
        // `ip:port` as the server sees us, without it NAT is assumed
        let seen_port = response
            .message
            .split_whitespace()
            .nth(1)
            .and_then(|address| address.rsplit_once(':'))
            .and_then(|(_, port)| port.parse::<u16>().ok());
        let transport = self.transport_mut()?;
        transport.session = Some(session.to_owned());
        let local_port = transport.local_port().ok();
        self.behind_nat = match (seen_port, local_port) {
            (Some(seen), Some(local)) => seen != local,
            _ => true,
        };
        self.credentials = Some(Credentials {
            user: user.to_owned(),
            pass: pass.to_owned(),
        });
        self.trace(|| format!("logged in, behind_nat={}", self.behind_nat));
        Ok(())
    }

    /// LOGOUT, which replies 203 LOGGED OUT or 403 NOT LOGGED IN. It only works
    /// when logged in, and fails with 403 when there is no session.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn logout(&mut self) -> Result<(), Error> {
        let result = self.call(
            &Request::new("LOGOUT"),
            &[ResponseCode::LoggedOut, ResponseCode::NotLoggedIn],
        );
        if let Ok(transport) = self.transport_mut() {
            transport.session = None;
        }
        self.credentials = None;
        self.trace(|| "session cleared by LOGOUT".to_owned());
        let response = result?;
        if response.code == ResponseCode::NotLoggedIn {
            return Err(Error::from_reply(
                ResponseCode::NotLoggedIn,
                &response.message,
            ));
        }
        Ok(())
    }

    /// The local UDP port of the open socket, `None` when not connected.
    #[must_use]
    pub fn local_port(&self) -> Option<u16> {
        self.transport.as_ref()?.local_port().ok()
    }

    /// Whether the client is behind NAT: the port `AniDB` saw in the AUTH reply
    /// differs from the local port. `true` until the first AUTH reply, and when
    /// the reply carries no port.
    #[must_use]
    pub fn behind_nat(&self) -> bool {
        self.behind_nat
    }

    /// How long until a keep-alive is due, `Some(Duration::ZERO)` when it is due
    /// now, `None` when there is nothing to keep alive (no session, or after
    /// LOGOUT, a disconnect or a ban).
    ///
    /// The idle time counts from the last reply. It is
    /// [`ClientConfig::ping_interval`] behind NAT, to keep the NAT mapping (and
    /// so the source port) alive, and [`ClientConfig::uptime_interval`]
    /// otherwise, to keep the session open.
    #[must_use]
    pub fn keep_alive_due(&self) -> Option<Duration> {
        if self.banned.is_some() || !self.is_logged_in() {
            return None;
        }
        let limit = if self.behind_nat {
            self.config.ping_interval
        } else {
            self.config.uptime_interval
        };
        Some(limit.saturating_sub(self.last_request.elapsed()))
    }

    /// Sends the keep-alive if it is due: PING behind NAT, UPTIME otherwise.
    /// Returns whether one was sent. Nothing is sent when it is not due or when
    /// there is no session, so it is cheap to call often.
    ///
    /// Every command calls this first, so it is only needed by a program that is
    /// idle between commands. A failed keep-alive is returned as the error of
    /// the request, the same way as for any other request.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn keep_alive(&mut self) -> Result<bool, Error> {
        if self.keep_alive_due() != Some(Duration::ZERO) {
            return Ok(false);
        }
        let command = if self.behind_nat { "PING" } else { "UPTIME" };
        self.trace(|| {
            format!(
                "keep-alive due after {} ms idle, sending {command}",
                self.last_request.elapsed().as_millis()
            )
        });
        if self.behind_nat {
            self.ping()?;
        } else {
            self.uptime()?;
        }
        Ok(true)
    }

    /// `PING [nat=1]`, which needs no session. Returns the reply (300 PONG, with
    /// the port the server sees on the first data line).
    ///
    /// Behind NAT a ping keeps the NAT mapping alive. The spec asks clients not
    /// to ping more often than necessary.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn ping(&mut self) -> Result<Response, Error> {
        self.call(
            &Request::new("PING").param("nat", "1"),
            &[ResponseCode::Pong],
        )
    }

    /// FILE by ed2k hash and size, decoded with the given masks. 320
    /// `NO_SUCH_FILE` when the hash is unknown, which is how new files are
    /// detected.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn file_by_hash(
        &mut self,
        size: u64,
        ed2k: &str,
        file: &FileFields,
        anime: &AnimeFields,
    ) -> Result<FileReply, Error> {
        let response = self.call(
            &file::request(ed2k, size, file, anime),
            &[ResponseCode::File],
        )?;
        FileReply::decode(first_line(&response)?, file, anime)
    }

    /// MYLIST by ed2k hash and size. 321 `NO_SUCH_ENTRY` when the file is not in
    /// the mylist.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_by_hash(&mut self, size: u64, ed2k: &str) -> Result<MylistEntry, Error> {
        let request = Request::new("MYLIST")
            .param("ed2k", ed2k)
            .param("size", size.to_string());
        let response = self.call(&request, &[ResponseCode::Mylist])?;
        MylistEntry::parse(first_line(&response)?)
    }

    /// MYLISTADD by ed2k hash and size. Returns 210 (added) or 311 (edited).
    /// 310 without `edit` is [`Error::FileAlreadyInMylist`] with the current entry.
    ///
    /// Files added after hashing should get state 1 (on hdd), unless the user
    /// chose otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_add_by_hash(
        &mut self,
        size: u64,
        ed2k: &str,
        options: MylistAdd,
    ) -> Result<ResponseCode, Error> {
        let mut request = Request::new("MYLISTADD")
            .param("ed2k", ed2k)
            .param("size", size.to_string());
        if options.edit {
            request = request.param("edit", "1");
        }
        if let Some(state) = options.state {
            request = request.param("state", state_code(state).to_string());
        }
        let response = self.call(
            &request,
            &[
                ResponseCode::MylistEntryAdded,
                ResponseCode::MylistEntryEdited,
                ResponseCode::FileAlreadyInMylist,
            ],
        )?;
        if response.code == ResponseCode::FileAlreadyInMylist {
            let entry = MylistEntry::parse(first_line(&response)?)?;
            return Err(Error::FileAlreadyInMylist {
                entry: Box::new(entry),
            });
        }
        Ok(response.code)
    }

    /// Sends any request and checks the reply: codes in `expected` and codes
    /// up to 305 are returned, other codes become an [`Error`]. This is the
    /// path every command uses, so a new command needs no other plumbing.
    ///
    /// When the server lost the session (501 `LOGIN_FIRST` or 506
    /// `INVALID_SESSION`) the client logs in again with the stored credentials
    /// and sends the request once more. The encryption ends with the session, so
    /// with an api key the login starts over with ENCRYPT, then an encrypted
    /// AUTH: the password is never sent in the clear. AUTH, ENCRYPT and LOGOUT are never
    /// retried that way. Bans (504, 555) are remembered and stop all later
    /// requests. Fatal server errors close the connection, see `is_fatal`.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn call(
        &mut self,
        request: &Request,
        expected: &[ResponseCode],
    ) -> Result<Response, Error> {
        // the keep-alive requests themselves, and the session commands, skip it
        if !matches!(
            request.command.as_str(),
            "AUTH" | "ENCRYPT" | "LOGOUT" | "PING" | "UPTIME"
        ) {
            self.keep_alive()?;
        }
        let error = match self.send(request, expected) {
            Ok(response) => return Ok(response),
            Err(e) => e,
        };

        // A reply that cannot be decrypted is the server's plain 501 or 506
        // after it dropped the encryption together with the session.
        let session_lost = matches!(
            &error,
            Error::Reply { code, .. }
                if matches!(code, ResponseCode::LoginFirst | ResponseCode::InvalidSession)
        ) || matches!(&error, Error::Decrypt);
        let relogin = !matches!(request.command.as_str(), "AUTH" | "ENCRYPT" | "LOGOUT");
        let error = match (&self.credentials, session_lost && relogin) {
            (Some(credentials), true) => {
                let Credentials { user, pass } = credentials.clone();
                self.warn("AniDB lost the session, logging in again");
                self.trace(|| {
                    format!(
                        "session lost on {}: {}, re-login started",
                        request.command,
                        describe_error(&error)
                    )
                });
                let relogin = self.relogin(&user, &pass);
                self.trace(|| match &relogin {
                    Ok(()) => format!("re-login finished, retrying {}", request.command),
                    Err(e) => format!("re-login failed: {}", describe_error(e)),
                });
                match relogin.and_then(|()| self.send(request, expected)) {
                    Ok(response) => {
                        self.trace(|| format!("retry of {} succeeded", request.command));
                        return Ok(response);
                    }
                    Err(e) => {
                        self.trace(|| {
                            format!(
                                "retry of {} failed: {}",
                                request.command,
                                describe_error(&e)
                            )
                        });
                        e
                    }
                }
            }
            _ => error,
        };

        match &error {
            Error::Banned { code, message } => {
                self.banned = Some((*code, message.clone()));
                self.close();
            }
            Error::Reply { code, .. } | Error::Server { code, .. } if is_fatal(*code) => {
                self.close();
            }
            _ => {}
        }
        Err(error)
    }

    fn send(&mut self, request: &Request, expected: &[ResponseCode]) -> Result<Response, Error> {
        if let Some((code, message)) = &self.banned {
            return Err(Error::from_reply(*code, message));
        }
        let response = self.transport_mut()?.request(request)?;
        self.last_request = Instant::now();
        let code = response.code;
        if expected.contains(&code) || code.code() <= 305 {
            return Ok(response);
        }
        Err(Error::from_reply(
            code,
            &error_message(code, &response.message),
        ))
    }

    fn transport_mut(&mut self) -> Result<&mut Transport, Error> {
        self.transport.as_mut().ok_or(Error::Reply {
            code: ResponseCode::NotConnected,
            message: ResponseCode::NotConnected.to_string(),
        })
    }

    /// Logs out (best effort) and closes the socket. Safe to call when not
    /// connected, and more than once.
    ///
    /// The spec asks for a LOGOUT whenever the client exits or expects to be
    /// idle for 30 minutes or more. It does not ask for a full AUTH, command,
    /// LOGOUT cycle per command. No LOGOUT is sent after a ban.
    pub fn disconnect(&mut self) {
        if self.is_logged_in() && self.banned.is_none() {
            // best effort, the connection is closed either way
            let _ = self.logout();
        }
        self.close();
    }

    fn close(&mut self) {
        if self.transport.is_some() {
            self.trace(|| "connection closed".to_owned());
        }
        self.transport = None;
        self.credentials = None;
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// Codes after which the connection is of no use anymore, so the client closes
/// it. 501 and 506 are first answered by logging in again (see [`Client::call`]),
/// they only close the connection when that does not help.
const fn is_fatal(code: ResponseCode) -> bool {
    matches!(
        code,
        ResponseCode::AccessDenied
            | ResponseCode::ClientVersionOutdated
            | ResponseCode::AnidbOutOfService
            | ResponseCode::LoginFirst
            | ResponseCode::InvalidSession
            | ResponseCode::ServerBusy
            | ResponseCode::TimeoutDelayAndResubmit
            | ResponseCode::InternalServerError
            | ResponseCode::NoData
            | ResponseCode::ApiViolation
    )
}

/// The server's own text, with a hint added for the codes the spec says to wait
/// on: bans and the service being out of order.
fn error_message(code: ResponseCode, message: &str) -> String {
    if code.asks_to_wait() {
        let text = if message.is_empty() {
            code.to_string()
        } else {
            message.to_owned()
        };
        format!("{text}. {WAIT_HINT}")
    } else {
        message.to_owned()
    }
}

/// The first data line of a reply, or an error when it has none.
pub(crate) fn first_line(response: &Response) -> Result<&[String], Error> {
    response
        .lines
        .first()
        .map(Vec::as_slice)
        .ok_or_else(|| Error::BadData(format!("{} reply has no data", response.code.code())))
}

fn first_word(text: &str) -> Option<&str> {
    text.split_whitespace().next()
}

/// Whether `name` follows the spec's rule for the client name: 4 to 16 lower
/// case letters (a-z).
#[must_use]
pub fn is_valid_client_name(name: &str) -> bool {
    (4..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_lowercase())
}

const fn state_code(state: MylistState) -> u8 {
    match state {
        MylistState::Unknown => 0,
        MylistState::InternalStorage => 1,
        MylistState::ExternalStorage => 2,
        MylistState::Deleted => 3,
        MylistState::RemoteStorage => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::Client;

    fn assert_send<T: Send>() {}

    #[test]
    fn client_is_send() {
        assert_send::<Client>();
    }
}
