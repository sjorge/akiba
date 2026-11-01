//! Live tests against the real `AniDB` server. Ignored by default, they need
//! credentials and network access, and send real requests (AUTH, FILE,
//! LOGOUT), spaced 5 seconds apart by the client.
//!
//! `AniDB` bans an IP that talks too fast or from many source ports, so the
//! tests keep to one session whatever the command line:
//!
//! - One test at a time, even without `--test-threads=1`: a process wide lock
//!   is held for the whole test.
//! - Each test ends with a pause of 5 seconds after its LOGOUT, so the next
//!   test starts at least one request interval after the last request.
//! - All tests bind the same local port, `AKIBA_LIVE_LOCAL_PORT` (default
//!   40137). A busy port is an error, the tests never fall back to another.
//! - After a ban (504 or 555) every later test fails at once, without sending
//!   anything. Then wait at least 30 minutes before the next run.
//!
//! Run it with `-- --test-threads=1` as well. One run takes a few minutes
//! because of the pauses (about 1.5 minutes without the API key, about 3.5 with
//! it). Run at most one live run per 30 minutes.
//!
//! A test whose variables are not all set prints which ones are missing and is
//! skipped (it passes and sends nothing), so a run without credentials is safe.
//!
//! The plain tests need `AKIBA_LIVE_USER`, `AKIBA_LIVE_PASS` and
//! `AKIBA_LIVE_CLIENT`. They never use an API key, even when
//! `AKIBA_LIVE_APIKEY` is set:
//!
//! - `auth_file_logout`: AUTH, one FILE lookup, LOGOUT.
//! - `read_only_commands`: the read-only commands, then LOGOUT.
//!
//! The encrypted tests need the same three variables plus `AKIBA_LIVE_APIKEY`
//! (the UDP API key of the profile page, not the password). Without the key
//! they print that they are skipped and pass:
//!
//! - `encrypted_auth_file_logout` and `encrypted_read_only_commands`: the two
//!   checks above over an encrypted session.
//! - `encrypted_session_recovers_after_session_loss`: a lookup, then a LOGOUT
//!   that the client does not know about, then a lookup that must recover the
//!   session (ENCRYPT, encrypted AUTH and one retry).
//!
//! ```text
//! AKIBA_LIVE_USER=.. AKIBA_LIVE_PASS=.. AKIBA_LIVE_CLIENT=.. [AKIBA_LIVE_APIKEY=..] \
//!   cargo test -p anidb-udp --test live -- --ignored --nocapture --test-threads=1
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use anidb_udp::commands::{IdOrName, RandomType, UpdatedSince};
use anidb_udp::constants::REQUEST_INTERVAL;
use anidb_udp::mask::{AnimeField, FileField};
use anidb_udp::{Client, ClientConfig, Error, Request, ResponseCode};

/// ed2k and size of a file known to `AniDB` (the fixture of the file reply test).
const ED2K: &str = "7dafd31d6d028e981d09301c5fbfc654";
const SIZE: u64 = 447_945_964;

/// The local port of every live test when `AKIBA_LIVE_LOCAL_PORT` is not set.
const DEFAULT_LOCAL_PORT: u16 = 40137;

/// The live tests take turns on the server, see [`turn`]. The lock is process
/// wide, so it holds even when the tests run in parallel threads.
static LIVE: Mutex<()> = Mutex::new(());

/// Set once `AniDB` has banned this IP, see [`Live::live`]. Later tests fail
/// before they send anything.
static BANNED: AtomicBool = AtomicBool::new(false);

/// The warnings the client logged, in order. They are also printed to stderr.
type Log = Arc<Mutex<Vec<String>>>;

/// A test's turn on the live server. It holds the lock until the test ends.
/// When it is dropped, also while a test panics, it waits one request
/// interval, so the next test's first request comes after this test's last one.
struct Turn {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for Turn {
    fn drop(&mut self) {
        // the lock is released only after this, when the fields are dropped
        thread::sleep(REQUEST_INTERVAL);
    }
}

/// Takes the turn at the start of a live test, before anything is sent. Panics
/// without a request when an earlier test saw a ban.
fn turn() -> Turn {
    let lock = LIVE.lock().unwrap_or_else(PoisonError::into_inner);
    if BANNED.load(Ordering::SeqCst) {
        drop(lock);
        panic!(
            "AniDB banned this IP earlier in this run, no request was sent. \
             Wait at least 30 minutes before the next live run."
        );
    }
    Turn { _lock: lock }
}

/// Unwraps the reply of a live request. A ban is remembered first, so that the
/// later tests stop before they send anything.
trait Live<T> {
    fn live(self) -> T;
}

impl<T> Live<T> for Result<T, Error> {
    fn live(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => {
                if matches!(error, Error::Banned { .. }) {
                    BANNED.store(true, Ordering::SeqCst);
                }
                panic!("live request failed: {error}");
            }
        }
    }
}

/// The live credentials from the environment.
struct Credentials {
    user: String,
    pass: String,
    client: String,
}

/// The credentials of a live test. When one of the variables is missing the
/// test prints which ones and is skipped (the caller returns), so a run without
/// credentials passes and sends nothing.
fn credentials_or_skip(test: &str) -> Option<Credentials> {
    let get = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let user = get("AKIBA_LIVE_USER");
    let pass = get("AKIBA_LIVE_PASS");
    let client = get("AKIBA_LIVE_CLIENT");
    match (user, pass, client) {
        (Some(user), Some(pass), Some(client)) => Some(Credentials { user, pass, client }),
        (user, pass, client) => {
            let missing: Vec<&str> = [
                ("AKIBA_LIVE_USER", user.is_none()),
                ("AKIBA_LIVE_PASS", pass.is_none()),
                ("AKIBA_LIVE_CLIENT", client.is_none()),
            ]
            .into_iter()
            .filter_map(|(name, absent)| absent.then_some(name))
            .collect();
            eprintln!("{test}: skipped, {} not set", missing.join(", "));
            None
        }
    }
}

/// The fixed local port of the live tests. Port 0 is refused, it would pick a
/// random port.
fn local_port() -> u16 {
    match std::env::var("AKIBA_LIVE_LOCAL_PORT") {
        Ok(text) => match text.parse::<u16>() {
            Ok(port) if port != 0 => port,
            _ => panic!("AKIBA_LIVE_LOCAL_PORT must be a port from 1 to 65535, not {text:?}"),
        },
        Err(std::env::VarError::NotPresent) => DEFAULT_LOCAL_PORT,
        Err(error) => panic!("AKIBA_LIVE_LOCAL_PORT: {error}"),
    }
}

/// The API key for an encrypted test. Without it the test prints that it is
/// skipped, and the caller returns.
fn api_key_or_skip(test: &str) -> Option<String> {
    let key = std::env::var("AKIBA_LIVE_APIKEY")
        .ok()
        .filter(|key| !key.is_empty());
    if key.is_none() {
        eprintln!("{test}: skipped, AKIBA_LIVE_APIKEY is not set");
    }
    key
}

/// A client logged in with the live credentials, on the fixed local port.
/// `api_key` turns on encryption.
fn login(credentials: &Credentials, api_key: Option<&str>) -> (Client, Log) {
    let mut config = ClientConfig::new(credentials.client.clone(), 1);
    config.local_port = Some(local_port());
    // a busy port is an error, never another port
    config.local_port_fallback = false;
    let mut client = Client::new(config);
    let log: Log = Arc::default();
    let sink = Arc::clone(&log);
    client.set_logger(move |message| {
        eprintln!("anidb: {message}");
        sink.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(message.to_owned());
    });
    client
        .connect(&credentials.user, &credentials.pass, api_key)
        .live();
    (client, log)
}

/// A FILE lookup of the known file, checking its size.
fn assert_known_file(client: &mut Client) {
    let file = [FileField::Aid, FileField::Size, FileField::Ed2k]
        .into_iter()
        .collect();
    let anime = [AnimeField::RomanjiName].into_iter().collect();
    let reply = client.file_by_hash(SIZE, ED2K, &file, &anime).live();
    assert_eq!(reply.file[&FileField::Size].as_int(), Some(447_945_964));
}

fn check_auth_file_logout(credentials: &Credentials, api_key: Option<&str>) {
    let (mut client, _) = login(credentials, api_key);
    assert_known_file(&mut client);
    client.logout().live();
}

/// The read-only commands against the real server. Only the reply shapes are
/// checked, since the data on the server changes.
fn check_read_only_commands(credentials: &Credentials, api_key: Option<&str>) {
    let (mut client, _) = login(credentials, api_key);

    // Cowboy Bebop
    let aid = 23;
    eprintln!("live: VERSION");
    assert_ne!(client.version().live(), "");
    eprintln!("live: UPTIME");
    assert!(client.uptime().live() > 0);
    eprintln!("live: ANIME");
    assert_eq!(client.anime(IdOrName::Id(aid), None).live().aid, aid);
    eprintln!("live: ANIMEDESC");
    assert_ne!(client.anime_desc(aid).live(), "");
    eprintln!("live: CALENDAR");
    client.calendar().live();
    eprintln!("live: GROUPSTATUS");
    client.groupstatus(aid, None).live();
    eprintln!("live: UPDATED");
    client.updated(UpdatedSince::Age(1)).live();
    eprintln!("live: MYLISTSTATS");
    client.mylist_stats().live();
    eprintln!("live: RANDOM");
    client.random(RandomType::Db).live();

    client.logout().live();
}

#[test]
#[ignore = "needs AniDB credentials and network access"]
fn auth_file_logout() {
    let Some(credentials) = credentials_or_skip("auth_file_logout") else {
        return;
    };
    let _turn = turn();
    check_auth_file_logout(&credentials, None);
}

#[test]
#[ignore = "needs AniDB credentials and network access"]
fn read_only_commands() {
    let Some(credentials) = credentials_or_skip("read_only_commands") else {
        return;
    };
    let _turn = turn();
    check_read_only_commands(&credentials, None);
}

#[test]
#[ignore = "needs AniDB credentials, an API key and network access"]
fn encrypted_auth_file_logout() {
    let Some(key) = api_key_or_skip("encrypted_auth_file_logout") else {
        return;
    };
    let Some(credentials) = credentials_or_skip("encrypted_auth_file_logout") else {
        return;
    };
    let _turn = turn();
    check_auth_file_logout(&credentials, Some(&key));
}

#[test]
#[ignore = "needs AniDB credentials, an API key and network access"]
fn encrypted_read_only_commands() {
    let Some(key) = api_key_or_skip("encrypted_read_only_commands") else {
        return;
    };
    let Some(credentials) = credentials_or_skip("encrypted_read_only_commands") else {
        return;
    };
    let _turn = turn();
    check_read_only_commands(&credentials, Some(&key));
}

/// The server forgets the session and its encryption, but the client still
/// believes it is logged in. The next lookup must log in again and succeed.
#[test]
#[ignore = "needs AniDB credentials, an API key and network access"]
fn encrypted_session_recovers_after_session_loss() {
    let Some(key) = api_key_or_skip("encrypted_session_recovers_after_session_loss") else {
        return;
    };
    let Some(credentials) = credentials_or_skip("encrypted_session_recovers_after_session_loss")
    else {
        return;
    };
    let _turn = turn();
    let (mut client, log) = login(&credentials, Some(&key));

    // the session works before the loss
    assert_known_file(&mut client);

    // A raw LOGOUT ends the session on the server only. The client waits the
    // rate-limit interval before it, as it does for every request.
    client
        .call(&Request::new("LOGOUT"), &[ResponseCode::LoggedOut])
        .live();
    assert!(
        client.is_logged_in(),
        "the client should still think it is logged in"
    );

    // The lookup gets 501 or 506, or a reply it cannot decrypt. The client then
    // logs in again (ENCRYPT, then encrypted AUTH) and retries the lookup once.
    assert_known_file(&mut client);
    assert!(
        log.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|message| message.contains("lost the session")),
        "the client should have logged in again after the session loss"
    );

    client.logout().live();
}
