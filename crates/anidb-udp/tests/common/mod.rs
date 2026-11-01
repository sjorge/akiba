//! Shared helpers of the client tests against a fake `AniDB` server on `127.0.0.1`.
//!
//! Every test that uses this module binds a real UDP socket on localhost. They need
//! no network beyond that, but a sandbox that forbids binding sockets (EPERM) fails
//! them all with `FakeServer::spawn: Operation not permitted`. They do not skip: run
//! them outside such a sandbox with `cargo test -p anidb-udp`.
// unused helpers per test binary are expected: each binary uses part of this module.
#![allow(dead_code)]

use anidb_udp::mask::{AnimeField, AnimeFields, FileField, FileFields, Value};
use anidb_udp::{Client, ClientConfig, Error, Key, ResponseCode};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::io::Write;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const API_KEY: &str = "secretkey";
pub(crate) const SALT: &str = "1a2b3c4d";
pub(crate) const ED2K: &str = "7dafd31d6d028e981d09301c5fbfc654";

pub(crate) const ENTRY: &str = "5|99|7|10914|3|1700000000|1|0|||ed2k|2";

/// A request as the fake server saw it, after decrypting.
#[derive(Debug, Clone)]
pub(crate) struct Incoming {
    pub(crate) command: String,
    pub(crate) params: HashMap<String, String>,
    pub(crate) tag: String,
    /// The line as sent, without the `tag` and `s` parameters.
    pub(crate) line: String,
    pub(crate) at: Instant,
    /// Whether the datagram was encrypted.
    pub(crate) encrypted: bool,
}

impl Incoming {
    pub(crate) fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }
}

/// What the fake server sends back.
pub(crate) enum Out {
    /// `{tag} {body}`
    Tagged(String),
    /// The same, deflated behind two zero bytes.
    Compressed(String),
    /// Exactly this text, no tag added.
    Raw(String),
}

type Handler = Box<dyn FnMut(&Incoming) -> Vec<Out> + Send>;

pub(crate) struct FakeServer {
    pub(crate) port: u16,
    pub(crate) seen: Arc<Mutex<Vec<Incoming>>>,
    pub(crate) stop: Arc<AtomicBool>,
    /// Set by a test to make the server time the session out at the next
    /// request: it forgets the encryption key and answers that request with a
    /// plain `506 INVALID SESSION`, which is what a real server does.
    pub(crate) timeout_session: Arc<AtomicBool>,
    pub(crate) thread: Option<JoinHandle<()>>,
}

impl FakeServer {
    /// `api_key` makes the server answer ENCRYPT and encrypt everything after it.
    pub(crate) fn spawn(api_key: Option<&'static str>, mut handler: Handler) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("FakeServer::spawn: cannot bind UDP");
        socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let port = socket.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let timeout_session = Arc::new(AtomicBool::new(false));
        let thread = {
            let (seen, stop) = (Arc::clone(&seen), Arc::clone(&stop));
            let timeout_session = Arc::clone(&timeout_session);
            thread::spawn(move || {
                let mut key: Option<Key> = None;
                let mut buffer = [0_u8; 4096];
                while !stop.load(Ordering::Relaxed) {
                    let Ok((length, peer)) = socket.recv_from(&mut buffer) else {
                        continue;
                    };
                    let at = Instant::now();
                    let data = match &key {
                        Some(key) => key.decrypt(&buffer[..length]).expect("client sent plain"),
                        None => buffer[..length].to_vec(),
                    };
                    let mut incoming = parse_request(&String::from_utf8(data).unwrap(), at);
                    incoming.encrypted = key.is_some();
                    seen.lock().unwrap().push(incoming.clone());

                    if timeout_session.swap(false, Ordering::Relaxed) {
                        key = None;
                        let text = format!("{} 506 INVALID SESSION", incoming.tag);
                        socket.send_to(text.as_bytes(), peer).unwrap();
                        continue;
                    }

                    if let (Some(api_key), "ENCRYPT") = (api_key, incoming.command.as_str()) {
                        let text = format!("{} 209 {SALT} ENCRYPTION ENABLED", incoming.tag);
                        socket.send_to(text.as_bytes(), peer).unwrap();
                        key = Some(Key::new(api_key, SALT));
                        continue;
                    }
                    for out in handler(&incoming) {
                        let bytes = match out {
                            Out::Tagged(body) => format!("{} {body}", incoming.tag).into_bytes(),
                            Out::Compressed(body) => deflate(&format!("{} {body}", incoming.tag)),
                            Out::Raw(text) => text.into_bytes(),
                        };
                        let bytes = match &key {
                            Some(key) => key.encrypt(&bytes),
                            None => bytes,
                        };
                        socket.send_to(&bytes, peer).unwrap();
                    }
                }
            })
        };
        Self {
            port,
            seen,
            stop,
            timeout_session,
            thread: Some(thread),
        }
    }

    pub(crate) fn client(&self, interval: Duration, timeout: Duration) -> Client {
        let mut config = ClientConfig::new("akibatest", 7);
        "127.0.0.1".clone_into(&mut config.host);
        config.port = self.port;
        config.interval = interval;
        config.timeout = timeout;
        Client::new(config)
    }

    pub(crate) fn fast_client(&self) -> Client {
        self.client(Duration::from_millis(1), Duration::from_secs(5))
    }

    pub(crate) fn commands(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.command.clone())
            .collect()
    }

    pub(crate) fn requests(&self) -> Vec<Incoming> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) fn parse_request(text: &str, at: Instant) -> Incoming {
    let (command, rest) = text.split_once(' ').unwrap_or((text, ""));
    let params: HashMap<String, String> = rest
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let pairs: Vec<&str> = rest
        .split('&')
        .filter(|pair| !pair.starts_with("tag=") && !pair.starts_with("s="))
        .collect();
    let line = format!("{command} {}", pairs.join("&"));
    Incoming {
        line,
        command: command.to_owned(),
        tag: params["tag"].clone(),
        params,
        at,
        encrypted: false,
    }
}

pub(crate) fn deflate(text: &str) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(vec![0, 0], Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

/// The reply of a well behaved server to the commands of the tests.
pub(crate) fn standard(request: &Incoming) -> Vec<Out> {
    let body = match request.command.as_str() {
        "AUTH" => "200 sess1 127.0.0.1:4242 LOGIN ACCEPTED".to_owned(),
        "LOGOUT" => "203 LOGGED OUT".to_owned(),
        "PING" => "300 PONG\n4242".to_owned(),
        "FILE" => "220 FILE\n99|10914|447945964|13".to_owned(),
        "MYLIST" => "221 MYLIST\n5|99|7|10914|3|1700000000|1|0|||ed2k|2".to_owned(),
        "MYLISTADD" if request.param("edit") == Some("1") => {
            "311 MYLIST ENTRY EDITED\n1".to_owned()
        }
        "MYLISTADD" => "210 MYLIST ENTRY ADDED\n5".to_owned(),
        other => format!("598 UNKNOWN COMMAND {other}"),
    };
    vec![Out::Tagged(body)]
}

pub(crate) fn standard_server() -> FakeServer {
    FakeServer::spawn(None, Box::new(standard))
}

pub(crate) fn file_fields() -> (FileFields, AnimeFields) {
    (
        [FileField::Aid, FileField::Size].into_iter().collect(),
        [AnimeField::AnimeTotalEpisodes].into_iter().collect(),
    )
}

pub(crate) fn assert_file_reply(client: &mut Client) {
    let (file, anime) = file_fields();
    let reply = client
        .file_by_hash(447_945_964, ED2K, &file, &anime)
        .unwrap();
    assert_eq!(reply.fid, 99);
    assert_eq!(reply.file[&FileField::Aid], Value::Int(10_914));
    assert_eq!(reply.file[&FileField::Size], Value::Int(447_945_964));
    assert_eq!(reply.anime[&AnimeField::AnimeTotalEpisodes], Value::Int(13));
}

/// Logs in, runs `call` against a server that answers every command with `reply`
/// (the code line and the data lines), and returns the line the command sent.
pub(crate) fn exchange(reply: &str, call: impl FnOnce(&mut Client)) -> String {
    let reply = reply.to_owned();
    let server = FakeServer::spawn(
        None,
        Box::new(move |request| match request.command.as_str() {
            "AUTH" | "LOGOUT" => standard(request),
            _ => vec![Out::Tagged(reply.clone())],
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    call(&mut client);
    drop(client);
    let requests = server.requests();
    let command = &requests[1];
    assert_ne!(command.command, "LOGOUT", "the command was not sent");
    assert_eq!(command.param("s"), Some("sess1"));
    command.line.clone()
}

pub(crate) fn expect_reply_error(error: &Error, code: ResponseCode) {
    assert!(
        matches!(error, Error::Reply { code: c, .. } if *c == code),
        "{error:?}"
    );
}

pub(crate) fn expect_invalid_argument(error: &Error) {
    assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
}
