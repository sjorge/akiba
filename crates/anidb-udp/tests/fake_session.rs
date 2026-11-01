//! Client tests against the fake `AniDB` server of `common`: sessions, encryption,
//! errors and bans, the local port, NAT and keep-alive.

mod common;

use common::{
    assert_file_reply, file_fields, standard, standard_server, FakeServer, Out, API_KEY, ED2K,
};

use std::net::UdpSocket;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use anidb_udp::mylist::MylistState;
use anidb_udp::{Client, ClientConfig, Error, MylistAdd, ResponseCode};

#[test]
fn plain_session_runs_all_typed_commands() {
    let server = standard_server();
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    assert!(client.is_logged_in());

    client.ping().unwrap();
    assert_file_reply(&mut client);
    let entry = client.mylist_by_hash(447_945_964, ED2K).unwrap();
    assert_eq!((entry.lid, entry.fid, entry.aid), (5, 99, 10_914));
    assert_eq!(entry.state, Some(MylistState::InternalStorage));
    let added = client
        .mylist_add_by_hash(447_945_964, ED2K, MylistAdd::default())
        .unwrap();
    assert_eq!(added, ResponseCode::MylistEntryAdded);
    let edited = client
        .mylist_add_by_hash(
            447_945_964,
            ED2K,
            MylistAdd {
                edit: true,
                state: Some(MylistState::ExternalStorage),
            },
        )
        .unwrap();
    assert_eq!(edited, ResponseCode::MylistEntryEdited);
    client.logout().unwrap();
    assert!(!client.is_logged_in());

    let requests = server.requests();
    let commands: Vec<&str> = requests.iter().map(|r| r.command.as_str()).collect();
    assert_eq!(
        commands,
        [
            "AUTH",
            "PING",
            "FILE",
            "MYLIST",
            "MYLISTADD",
            "MYLISTADD",
            "LOGOUT"
        ]
    );
    let auth = &requests[0];
    assert_eq!(auth.param("user"), Some("me"));
    assert_eq!(auth.param("pass"), Some("pw"));
    assert_eq!(auth.param("protover"), Some("3"));
    assert_eq!(auth.param("client"), Some("akibatest"));
    assert_eq!(auth.param("clientver"), Some("7"));
    assert_eq!(auth.param("comp"), Some("1"));
    assert_eq!(auth.param("nat"), Some("1"));
    assert_eq!(auth.param("s"), None);
    let file = &requests[2];
    assert_eq!(file.param("s"), Some("sess1"));
    assert_eq!(file.param("ed2k"), Some(ED2K));
    assert_eq!(file.param("size"), Some("447945964"));
    assert_eq!(file.param("fmask"), Some(file_fields().0.to_hex().as_str()));
    assert_eq!(file.param("amask"), Some(file_fields().1.to_hex().as_str()));
    assert_eq!(requests[4].param("edit"), None);
    assert_eq!(requests[5].param("edit"), Some("1"));
    assert_eq!(requests[5].param("state"), Some("2"));
}

#[test]
fn encrypted_session_encrypts_both_ways() {
    let server = FakeServer::spawn(Some(API_KEY), Box::new(standard));
    let mut client = server.fast_client();
    client.connect("me", "pw", Some(API_KEY)).unwrap();
    assert_file_reply(&mut client);
    client.logout().unwrap();
    // the fake server fails on any plain request after ENCRYPT
    assert_eq!(server.commands(), ["ENCRYPT", "AUTH", "FILE", "LOGOUT"]);
    let encrypt = &server.requests()[0];
    assert_eq!(encrypt.param("user"), Some("me"));
    assert_eq!(encrypt.param("type"), Some("1"));
}

#[test]
fn compressed_reply_is_inflated() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "FILE" => vec![Out::Compressed(
                "220 FILE\n99|10914|447945964|13".to_owned(),
            )],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    assert_file_reply(&mut client);
}

#[test]
fn compressed_reply_inside_an_encrypted_session() {
    let server = FakeServer::spawn(
        Some(API_KEY),
        Box::new(|request| match request.command.as_str() {
            "FILE" => vec![Out::Compressed(
                "220 FILE\n99|10914|447945964|13".to_owned(),
            )],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", Some(API_KEY)).unwrap();
    assert_file_reply(&mut client);
}

#[test]
fn no_reply_is_a_timeout() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "PING" => Vec::new(),
            _ => standard(request),
        }),
    );
    let mut client = server.client(Duration::from_millis(1), Duration::from_millis(150));
    client.connect("me", "pw", None).unwrap();
    let started = Instant::now();
    let error = client.ping().unwrap_err();
    assert!(matches!(error, Error::Timeout { .. }), "{error:?}");
    assert!(started.elapsed() >= Duration::from_millis(140));
    // a timeout is not a ban or a lost session: the client still works
    assert_file_reply(&mut client);
}

#[test]
fn reply_with_a_foreign_tag_is_dropped() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "FILE" => vec![
                Out::Raw("ZZZZZ 320 NO SUCH FILE".to_owned()),
                Out::Raw("garbage".to_owned()),
                Out::Tagged("220 FILE\n99|10914|447945964|13".to_owned()),
            ],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    assert_file_reply(&mut client);
}

#[test]
fn invalid_session_logs_in_again_and_retries_once() {
    let mut file_calls = 0;
    let mut auths = 0;
    let server = FakeServer::spawn(
        None,
        Box::new(move |request| match request.command.as_str() {
            "AUTH" => {
                auths += 1;
                vec![Out::Tagged(format!(
                    "200 sess{auths} 127.0.0.1:1 LOGIN ACCEPTED"
                ))]
            }
            "FILE" => {
                file_calls += 1;
                if file_calls == 1 {
                    vec![Out::Tagged("506 INVALID SESSION".to_owned())]
                } else {
                    standard(request)
                }
            }
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    assert_file_reply(&mut client);

    let requests = server.requests();
    let sequence: Vec<(&str, Option<&str>)> = requests
        .iter()
        .map(|r| (r.command.as_str(), r.param("s")))
        .collect();
    assert_eq!(
        sequence,
        [
            ("AUTH", None),
            ("FILE", Some("sess1")),
            ("AUTH", None),
            ("FILE", Some("sess2")),
        ]
    );
}

#[test]
fn encrypted_session_timeout_encrypts_again_before_logging_in() {
    let server = FakeServer::spawn(Some(API_KEY), Box::new(standard));
    let mut client = server.fast_client();
    client.connect("me", "pw", Some(API_KEY)).unwrap();
    assert_file_reply(&mut client);

    // the session times out: the server drops the encryption, and answers the
    // next (encrypted) request in the clear
    server.timeout_session.store(true, Ordering::Relaxed);
    assert_file_reply(&mut client);
    assert!(client.is_logged_in());
    // the connection is encrypted again afterwards
    assert_file_reply(&mut client);
    client.logout().unwrap();

    let requests = server.requests();
    let sequence: Vec<(&str, bool)> = requests
        .iter()
        .map(|r| (r.command.as_str(), r.encrypted))
        .collect();
    assert_eq!(
        sequence,
        [
            ("ENCRYPT", false),
            ("AUTH", true),
            ("FILE", true),
            // lost session: answered with a plain 506
            ("FILE", true),
            // the re-login starts over like the first one: ENCRYPT, encrypted AUTH
            ("ENCRYPT", false),
            ("AUTH", true),
            ("FILE", true),
            ("FILE", true),
            ("LOGOUT", true),
        ]
    );
    // the password never goes out in the clear
    for request in &requests {
        assert!(
            request.encrypted || request.param("pass").is_none(),
            "{request:?}"
        );
    }
    // ENCRYPT carries no session
    assert!(requests
        .iter()
        .filter(|r| r.command == "ENCRYPT")
        .all(|r| r.param("s").is_none()));
}

#[test]
fn a_second_session_loss_is_an_error() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "FILE" => vec![Out::Tagged("501 LOGIN FIRST".to_owned())],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    let (file, anime) = file_fields();
    let error = client.file_by_hash(1, ED2K, &file, &anime).unwrap_err();
    assert!(
        matches!(
            error,
            Error::Reply {
                code: ResponseCode::LoginFirst,
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(server.commands(), ["AUTH", "FILE", "AUTH", "FILE"]);
}

#[test]
fn untagged_ban_is_typed_and_stops_further_requests() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "FILE" => vec![Out::Raw("555 BANNED\nflooding".to_owned())],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    let (file, anime) = file_fields();
    let error = client.file_by_hash(1, ED2K, &file, &anime).unwrap_err();
    assert!(matches!(error, Error::Banned { .. }), "{error:?}");
    let error = client.ping().unwrap_err();
    assert!(matches!(error, Error::Banned { .. }), "{error:?}");
    drop(client);
    // no PING, and no LOGOUT for a server that banned us
    assert_eq!(server.commands(), ["AUTH", "FILE"]);
}

#[test]
fn server_errors_are_typed() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "FILE" => vec![Out::Raw(
                "601 ANIDB OUT OF SERVICE - PLEASE TRY AGAIN LATER".to_owned(),
            )],
            "MYLIST" => vec![Out::Tagged("321 NO SUCH ENTRY".to_owned())],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    let (file, anime) = file_fields();
    let error = client.file_by_hash(1, ED2K, &file, &anime).unwrap_err();
    assert!(matches!(error, Error::Server { .. }), "{error:?}");
    // a fatal error closes the connection
    assert!(!client.is_logged_in());
    client.connect("me", "pw", None).unwrap();
    let error = client.mylist_by_hash(1, ED2K).unwrap_err();
    assert!(
        matches!(
            error,
            Error::Reply {
                code: ResponseCode::NoSuchEntry,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn login_failure_is_an_error_and_leaves_nothing_open() {
    let server = FakeServer::spawn(
        None,
        Box::new(|_| vec![Out::Tagged("500 LOGIN FAILED".to_owned())]),
    );
    let mut client = server.fast_client();
    let error = client.connect("me", "nope", None).unwrap_err();
    assert!(
        matches!(
            error,
            Error::Reply {
                code: ResponseCode::LoginFailed,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(!client.is_logged_in());
    drop(client);
    assert_eq!(server.commands(), ["AUTH"]);
}

#[test]
fn request_starts_are_spaced_by_the_interval() {
    let server = standard_server();
    let interval = Duration::from_millis(50);
    let mut client = server.client(interval, Duration::from_secs(5));
    client.connect("me", "pw", None).unwrap();
    for _ in 0..3 {
        client.ping().unwrap();
    }
    let times: Vec<Instant> = server.requests().iter().map(|r| r.at).collect();
    assert_eq!(times.len(), 4);
    for pair in times.windows(2) {
        // arrival jitter on the server side, the sending side never goes below 50ms
        assert!(
            pair[1] - pair[0] >= Duration::from_millis(40),
            "{:?}",
            pair[1] - pair[0]
        );
    }
}

#[test]
fn logger_receives_warnings() {
    let server = standard_server();
    let mut config = ClientConfig::new("Bad Name", 1);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    let mut client = Client::new(config);
    let messages = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = std::sync::Arc::clone(&messages);
    client.set_logger(move |m| sink.lock().unwrap().push(m.to_owned()));
    client.connect("me", "pw", None).unwrap();
    assert!(messages.lock().unwrap()[0].contains("Bad Name"));
}

#[test]
fn dropping_a_logged_in_client_sends_logout() {
    let server = standard_server();
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    drop(client);
    assert_eq!(server.commands(), ["AUTH", "LOGOUT"]);
}

// ---- fixed local port, NAT detection and keep-alive

/// A UDP port that is free right now. Another process could grab it before the
/// test uses it, which is unlikely enough for a test.
fn free_port() -> u16 {
    UdpSocket::bind("0.0.0.0:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Collects the warnings of a client.
fn collect_warnings(client: &mut Client) -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
    let messages = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = std::sync::Arc::clone(&messages);
    client.set_logger(move |m| sink.lock().unwrap().push(m.to_owned()));
    messages
}

/// A server that tells the client it is seen on `seen_port`, and answers PING
/// and UPTIME.
fn nat_server(seen_port: u16) -> FakeServer {
    FakeServer::spawn(
        None,
        Box::new(move |request| match request.command.as_str() {
            "AUTH" => vec![Out::Tagged(format!(
                "200 sess1 127.0.0.1:{seen_port} LOGIN ACCEPTED"
            ))],
            "UPTIME" => vec![Out::Tagged("208 UPTIME\n123456".to_owned())],
            _ => standard(request),
        }),
    )
}

#[test]
fn fixed_local_port_is_bound() {
    let server = standard_server();
    let port = free_port();
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    config.local_port = Some(port);
    let mut client = Client::new(config);
    assert_eq!(client.local_port(), None);
    client.connect("me", "pw", None).unwrap();
    assert_eq!(client.local_port(), Some(port));
}

#[test]
fn busy_local_port_falls_back_with_a_warning() {
    let server = standard_server();
    let holder = UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = holder.local_addr().unwrap().port();
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    config.local_port = Some(port);
    let mut client = Client::new(config);
    let warnings = collect_warnings(&mut client);
    client.connect("me", "pw", None).unwrap();
    assert_ne!(client.local_port(), Some(port));
    assert!(client.local_port().is_some());
    let warnings = warnings.lock().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains(&port.to_string()), "{warnings:?}");
    assert!(warnings[0].contains("already in use"), "{warnings:?}");
}

#[test]
fn busy_local_port_without_fallback_is_an_error() {
    let server = standard_server();
    let holder = UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = holder.local_addr().unwrap().port();
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    config.local_port = Some(port);
    config.local_port_fallback = false;
    let mut client = Client::new(config);
    let error = client.connect("me", "pw", None).unwrap_err();
    assert!(
        matches!(&error, Error::Io(e) if e.kind() == std::io::ErrorKind::AddrInUse),
        "{error:?}"
    );
    assert!(error.to_string().contains(&port.to_string()), "{error}");
    assert_eq!(server.commands().len(), 0);
}

#[test]
fn nat_is_detected_from_the_auth_reply() {
    // the server sees another port than the local one: NAT
    let server = nat_server(4242);
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    assert!(client.behind_nat());
    let auth = &server.requests()[0];
    assert_eq!(auth.param("nat"), Some("1"));

    // the server sees the local port itself: no NAT
    let port = free_port();
    let server = nat_server(port);
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    config.local_port = Some(port);
    let mut client = Client::new(config);
    assert!(client.behind_nat(), "NAT is assumed before AUTH");
    client.connect("me", "pw", None).unwrap();
    assert!(!client.behind_nat());
}

#[test]
fn nat_is_assumed_when_the_auth_reply_has_no_port() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "AUTH" => vec![Out::Tagged("200 sess1 LOGIN ACCEPTED".to_owned())],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    assert!(client.behind_nat());
}

#[test]
fn keep_alive_sends_ping_behind_nat_when_due_and_only_then() {
    let server = nat_server(4242);
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    config.ping_interval = Duration::from_millis(200);
    config.uptime_interval = Duration::from_secs(3600);
    let mut client = Client::new(config);
    assert_eq!(client.keep_alive_due(), None);
    assert!(!client.keep_alive().unwrap());

    client.connect("me", "pw", None).unwrap();
    let wait = client.keep_alive_due().unwrap();
    assert!(wait > Duration::ZERO && wait <= Duration::from_millis(200));
    assert!(!client.keep_alive().unwrap());
    assert_eq!(server.commands(), ["AUTH"]);

    thread::sleep(Duration::from_millis(250));
    assert_eq!(client.keep_alive_due(), Some(Duration::ZERO));
    assert!(client.keep_alive().unwrap());
    assert_eq!(server.commands(), ["AUTH", "PING"]);
    assert_eq!(server.requests()[1].param("nat"), Some("1"));

    // the ping was activity, nothing is due right after it
    assert!(!client.keep_alive().unwrap());
    assert_eq!(server.commands(), ["AUTH", "PING"]);
}

#[test]
fn keep_alive_sends_uptime_without_nat() {
    let port = free_port();
    let server = nat_server(port);
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    config.local_port = Some(port);
    config.ping_interval = Duration::from_secs(3600);
    config.uptime_interval = Duration::from_millis(200);
    let mut client = Client::new(config);
    client.connect("me", "pw", None).unwrap();
    assert!(!client.behind_nat());

    assert!(!client.keep_alive().unwrap());
    thread::sleep(Duration::from_millis(250));
    assert!(client.keep_alive().unwrap());
    assert_eq!(server.commands(), ["AUTH", "UPTIME"]);
}

#[test]
fn a_command_sends_the_due_keep_alive_first() {
    let server = nat_server(4242);
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    config.ping_interval = Duration::from_millis(200);
    let mut client = Client::new(config);
    client.connect("me", "pw", None).unwrap();

    // not due yet: the command goes out alone
    client.mylist_by_hash(10914, ED2K).unwrap();
    assert_eq!(server.commands(), ["AUTH", "MYLIST"]);

    thread::sleep(Duration::from_millis(250));
    client.mylist_by_hash(10914, ED2K).unwrap();
    assert_eq!(server.commands(), ["AUTH", "MYLIST", "PING", "MYLIST"]);
}

#[test]
fn no_keep_alive_after_logout_or_ban() {
    let server = nat_server(4242);
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = server.port;
    config.interval = Duration::from_millis(1);
    config.ping_interval = Duration::from_millis(50);
    let mut client = Client::new(config);
    client.connect("me", "pw", None).unwrap();
    client.logout().unwrap();
    thread::sleep(Duration::from_millis(100));
    assert_eq!(client.keep_alive_due(), None);
    assert!(!client.keep_alive().unwrap());
    assert_eq!(server.commands(), ["AUTH", "LOGOUT"]);

    let banned = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "AUTH" => standard(request),
            _ => vec![Out::Raw("555 BANNED test".to_owned())],
        }),
    );
    let mut config = ClientConfig::new("akibatest", 7);
    "127.0.0.1".clone_into(&mut config.host);
    config.port = banned.port;
    config.interval = Duration::from_millis(1);
    config.ping_interval = Duration::from_millis(50);
    let mut client = Client::new(config);
    client.connect("me", "pw", None).unwrap();
    client.ping().unwrap_err();
    assert_eq!(client.keep_alive_due(), None);
}
