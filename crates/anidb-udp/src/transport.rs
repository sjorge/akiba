//! The UDP socket: one request in flight, replies matched by tag.
//!
//! This is the transport layer of the specification: one datagram per command,
//! replies of at most 1400 bytes, the tag that matches a reply to its request,
//! and the flood protection between requests. See the crate root for the link.

use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::client::ClientConfig;
use crate::codes::ResponseCode;
use crate::compression::{decompress, is_compressed};
use crate::crypto::Key;
use crate::error::Error;
use crate::rate_limiter::RateLimiter;
use crate::trace::{describe_reply, redact_params, Trace};
use crate::wire::{make_tag, parse, Request, Response, TAG_LENGTH};

/// Receives warnings and notes of the client; the caller decides where they go.
pub type Logger = Arc<dyn Fn(&str) + Send + Sync>;

/// Largest datagram read. Replies are at most 1400 bytes, the default MTU of
/// the spec. The spec allows a smaller MTU, down to 400 bytes.
const RECV_BUFFER: usize = 4096;

/// One UDP socket, connected to the API host, and the request/reply handling on
/// top of it. Every command is one datagram. Requests go through a
/// [`RateLimiter`], so only one request is in flight at a time.
pub struct Transport {
    socket: UdpSocket,
    peer: SocketAddr,
    host: String,
    limiter: RateLimiter,
    timeout: Duration,
    /// Session key, sent as `s` with every request when set.
    pub session: Option<String>,
    /// When set, all requests are encrypted and all replies decrypted.
    pub key: Option<Key>,
    log: Option<Logger>,
    trace: Option<Trace>,
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transport")
            .field("peer", &self.peer)
            .field("session", &self.session.is_some())
            .field("encrypted", &self.key.is_some())
            .finish_non_exhaustive()
    }
}

impl Transport {
    /// Resolves `config.host` once (the first IPv4 address), binds
    /// `0.0.0.0:<config.local_port>` and connects the socket to it. The request
    /// interval and timeout are taken from `config` too.
    ///
    /// The spec asks for a fixed local port, since too many different local
    /// ports from one IP get it banned. With `local_port` `None` the system
    /// picks one. When the fixed port is busy, `local_port_fallback` decides:
    /// `true` warns through `log` and binds a system picked port, `false` is an
    /// error that names the port.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the host cannot be resolved or the socket cannot be bound or connected, also when a busy fixed port has no fallback.
    pub fn open(config: &ClientConfig, log: Option<Logger>) -> Result<Self, Error> {
        let host = config.host.as_str();
        let peer = (host, config.port)
            .to_socket_addrs()?
            .find(SocketAddr::is_ipv4)
            .ok_or_else(|| {
                std::io::Error::new(ErrorKind::NotFound, format!("no IPv4 address for {host}"))
            })?;
        let socket = bind_local(config.local_port, config.local_port_fallback, log.as_ref())?;
        socket.connect(peer)?;
        Ok(Self {
            socket,
            peer,
            host: host.to_owned(),
            limiter: RateLimiter::new(config.interval),
            timeout: config.timeout,
            session: None,
            key: None,
            log,
            trace: None,
        })
    }

    /// Sets the callback for warnings.
    pub fn set_logger(&mut self, log: Option<Logger>) {
        self.log = log;
    }

    /// Sets the callback for trace lines, or turns tracing off with `None`.
    /// `since` is the time the milliseconds of the lines count from, the
    /// creation of the client for a [`crate::Client`]. Trace lines are verbose,
    /// one per event, and never contain a password, an API key, a salt, a
    /// session key or ciphertext, see the README. The first line names the
    /// local port.
    pub fn set_trace(&mut self, log: Option<Logger>, since: Instant) {
        self.trace = log.map(|sink| Trace::new(sink, since));
        let port = self.local_port().ok();
        self.trace(|| format!("tracing on: local port {port:?}, peer {}", self.peer));
    }

    /// Passes a trace line, which is only built when tracing is on.
    pub(crate) fn trace(&self, line: impl FnOnce() -> String) {
        if let Some(trace) = &self.trace {
            trace.emit(&line());
        }
    }

    pub(crate) fn warn(&self, message: &str) {
        if let Some(log) = &self.log {
            log(message);
        }
    }

    /// The local port the socket is bound to.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the socket has no local address.
    pub fn local_port(&self) -> Result<u16, Error> {
        Ok(self.socket.local_addr()?.port())
    }

    /// Sends one request and waits for its reply.
    ///
    /// Waits for the rate limiter, then encodes, encrypts and sends. Replies
    /// with another tag are dropped. Replies without a tag are only accepted
    /// for 504, 555 and 6xx, which the server sends untagged. A reply is then
    /// decrypted, decompressed and parsed. No reply within the timeout is
    /// [`Error::Timeout`]. The reply is returned whatever its code is.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] when no reply arrives after all retries, [`Error::BadResponse`] for a reply that cannot be parsed, and [`Error::Io`] for socket failures.
    pub fn request(&mut self, request: &Request) -> Result<Response, Error> {
        let waited = self.limiter.wait();
        if !waited.is_zero() {
            self.trace(|| {
                format!(
                    "waited {} ms before {}",
                    waited.as_millis(),
                    request.command
                )
            });
        }

        let tag = make_tag();
        let text = request.encode(self.session.as_deref(), &tag);
        let payload = match &self.key {
            Some(key) => key.encrypt(text.as_bytes()),
            None => text.into_bytes(),
        };
        self.socket.send(&payload)?;
        let sent = Instant::now();
        self.trace(|| {
            format!(
                "send {} tag={tag} encrypted={} length={} params={}",
                request.command,
                self.key.is_some(),
                payload.len(),
                redact_params(request, self.session.is_some(), &tag)
            )
        });

        let deadline = sent + self.timeout;
        let mut buffer = [0_u8; RECV_BUFFER];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(self.timed_out(request, sent));
            }
            self.socket.set_read_timeout(Some(left))?;
            let length = match self.socket.recv(&mut buffer) {
                Ok(length) => length,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Err(self.timed_out(request, sent));
                }
                Err(e) => return Err(Error::Io(e)),
            };
            if let Some(response) = self.handle(&buffer[..length], &tag, sent)? {
                return Ok(response);
            }
        }
    }

    fn timed_out(&self, request: &Request, sent: Instant) -> Error {
        // a request is never sent again by the transport
        self.trace(|| {
            format!(
                "timeout: {} got no reply after {} ms, retries=0",
                request.command,
                sent.elapsed().as_millis()
            )
        });
        Error::Timeout {
            message: format!(
                "{} no reply from {} within {:?}",
                ResponseCode::ConnectionTimeout,
                self.host,
                self.timeout
            ),
        }
    }

    /// Reads one datagram. `None` means it is not the reply to `tag`.
    ///
    /// A reply may be encrypted (after ENCRYPT) and compressed (two zero bytes
    /// first, after `comp=1`). It is decrypted first, then decompressed. The tag
    /// in front of it says which request it answers.
    fn handle(
        &mut self,
        datagram: &[u8],
        tag: &str,
        sent: Instant,
    ) -> Result<Option<Response>, Error> {
        let mut data = datagram.to_vec();
        let encrypted = self.key.is_some();
        if let Some(key) = &self.key {
            match key.decrypt(&data) {
                Ok(plain) => data = plain,
                Err(e) => {
                    // only the length of the ciphertext is traced, and the
                    // bytes as plain text when they parse as a reply
                    self.trace(|| {
                        format!(
                            "recv length={} after {} ms: decrypt failed, as a plain reply: {}",
                            datagram.len(),
                            sent.elapsed().as_millis(),
                            describe_reply(&String::from_utf8_lossy(datagram))
                        )
                    });
                    self.trace(|| "encryption turned off for this connection".to_owned());
                    // most likely AniDB dropped encryption, so stop using it
                    self.key = None;
                    self.warn(
                        "AniDB sent a reply that cannot be decrypted, encryption is turned off for the rest of this connection",
                    );
                    return Err(e);
                }
            }
        }
        let compressed = is_compressed(&data);
        if compressed {
            data = decompress(&data)?;
        }
        let text = String::from_utf8_lossy(&data);
        let matched = |text: &str| {
            format!(
                "recv length={} after {} ms decrypted={encrypted} compressed={compressed} {}",
                datagram.len(),
                sent.elapsed().as_millis(),
                describe_reply(text)
            )
        };

        let tagged = text.len() > TAG_LENGTH
            && text.is_char_boundary(TAG_LENGTH)
            && &text[..TAG_LENGTH] == tag
            && text.as_bytes()[TAG_LENGTH] == b' ';
        if tagged || is_untagged_error(&text) {
            self.trace(|| matched(&text));
            return parse(&text).map(Some);
        }
        // a reply to an earlier request that timed out, or noise
        self.trace(|| format!("{} (dropped, not the reply to tag {tag})", matched(&text)));
        Ok(None)
    }
}

/// Binds the local socket, see [`Transport::open`].
fn bind_local(
    local_port: Option<u16>,
    fallback: bool,
    log: Option<&Logger>,
) -> Result<UdpSocket, Error> {
    let Some(wanted) = local_port.filter(|port| *port != 0) else {
        return Ok(UdpSocket::bind("0.0.0.0:0")?);
    };
    match UdpSocket::bind(("0.0.0.0", wanted)) {
        Ok(socket) => Ok(socket),
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            let message = format!("local UDP port {wanted} is already in use");
            if !fallback {
                return Err(Error::Io(std::io::Error::new(
                    ErrorKind::AddrInUse,
                    message,
                )));
            }
            if let Some(log) = log {
                log(&format!("{message}, using a random port instead"));
            }
            Ok(UdpSocket::bind("0.0.0.0:0")?)
        }
        Err(e) => Err(Error::Io(e)),
    }
}

/// 504 and 555 (bans) and 6xx (server errors) come without a tag. Only one
/// request is ever in flight, so the reply belongs to it.
fn is_untagged_error(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() > 3
        && bytes[3] == b' '
        && (text.starts_with("504")
            || text.starts_with("555")
            || (bytes[0] == b'6' && bytes[1].is_ascii_digit() && bytes[2].is_ascii_digit()))
}
