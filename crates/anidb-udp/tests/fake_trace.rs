//! Trace lines against the fake `AniDB` server: the events that are traced, and
//! that no secret ever reaches a line.

mod common;

use common::{assert_file_reply, standard, FakeServer, Out, API_KEY, SALT};

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use anidb_udp::{Client, Request};

const PASSWORD: &str = "hunter2pw";
const SESSION: &str = "Zs3cr3tK";

/// A server that hands out a recognisable session key, so a leak can be found.
fn server(api_key: Option<&'static str>) -> FakeServer {
    FakeServer::spawn(
        api_key,
        Box::new(|request| match request.command.as_str() {
            "AUTH" => vec![Out::Tagged(format!(
                "200 {SESSION} 127.0.0.1:1 LOGIN ACCEPTED"
            ))],
            _ => standard(request),
        }),
    )
}

fn collect_trace(client: &mut Client) -> Arc<Mutex<Vec<String>>> {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&lines);
    client.set_trace(move |line| sink.lock().unwrap().push(line.to_owned()));
    lines
}

/// Sends a custom request whose parameters are named like secrets.
fn secret_named_ping(client: &mut Client) {
    let request = Request::new("PING")
        .param("nat", "1")
        .param("apikey", "LEAKAPIKEY")
        .param("salt", "LEAKSALT")
        .param("Secret_Token", "LEAKTOKEN")
        .param("mykey", "LEAKMYKEY");
    client.call(&request, &[]).unwrap();
}

const LEAKS: [&str; 8] = [
    PASSWORD,
    API_KEY,
    SALT,
    SESSION,
    "LEAKAPIKEY",
    "LEAKSALT",
    "LEAKTOKEN",
    "LEAKMYKEY",
];

fn assert_no_secrets(lines: &[String]) {
    for line in lines {
        for leak in LEAKS {
            assert!(!line.contains(leak), "{leak} leaked in {line}");
        }
        // the user name and the address the server sees are personal too
        for personal in ["user=me", "127.0.0.1:1"] {
            assert!(!line.contains(personal), "{personal} leaked in {line}");
        }
    }
}

#[test]
fn encrypted_session_loss_is_traced_without_any_secret() {
    let server = server(Some(API_KEY));
    let mut client = server.fast_client();
    let lines = collect_trace(&mut client);
    client.connect("me", PASSWORD, Some(API_KEY)).unwrap();
    assert_file_reply(&mut client);
    secret_named_ping(&mut client);

    // the server drops the session and the key, and answers in the clear
    server.timeout_session.store(true, Ordering::Relaxed);
    assert_file_reply(&mut client);
    client.logout().unwrap();

    let lines = lines.lock().unwrap().clone();
    assert_no_secrets(&lines);
    let text = lines.join("\n");
    // the secrets are replaced, not dropped
    for marker in ["pass=***", "&s=***", "apikey=***", "salt=***", "mykey=***"] {
        assert!(text.contains(marker), "{marker} missing in\n{text}");
    }
    assert!(text.contains("message=\"*** *** LOGIN ACCEPTED\""));
    assert!(text.contains("message=\"*** ENCRYPTION ENABLED\""));
    // the events
    assert!(
        lines[0].contains("tracing on: local port Some("),
        "{}",
        lines[0]
    );
    for event in [
        "send ENCRYPT",
        "send AUTH",
        "encrypted=true",
        "encryption key set",
        "login step ENCRYPT",
        "logged in",
        "decrypt failed, as a plain reply: code=506",
        "encryption turned off for this connection",
        "session lost on FILE",
        "encryption key dropped",
        "re-login step ENCRYPT",
        "re-login step AUTH",
        "re-login finished, retrying FILE",
        "retry of FILE succeeded",
        "session cleared by LOGOUT",
    ] {
        assert!(text.contains(event), "{event} missing in\n{text}");
    }
}

#[test]
fn plain_session_is_traced_without_the_password_or_session() {
    let server = server(None);
    let mut client = server.fast_client();
    let lines = collect_trace(&mut client);
    client.connect("me", PASSWORD, None).unwrap();
    assert_file_reply(&mut client);
    secret_named_ping(&mut client);
    client.logout().unwrap();

    let lines = lines.lock().unwrap().clone();
    assert_no_secrets(&lines);
    let text = lines.join("\n");
    assert!(text.contains("pass=***"));
    assert!(text.contains("&s=***"));
    assert!(text.contains("send FILE"));
    assert!(text.contains("encrypted=false"));
    assert!(text.contains("code=220"));
}

#[test]
fn a_timeout_is_traced() {
    let server = FakeServer::spawn(None, Box::new(|_| Vec::new()));
    let mut client = server.client(
        std::time::Duration::from_millis(1),
        std::time::Duration::from_millis(200),
    );
    let lines = collect_trace(&mut client);
    assert!(client.connect("me", PASSWORD, None).is_err());
    let text = lines.lock().unwrap().join("\n");
    assert!(text.contains("timeout: AUTH got no reply after"), "{text}");
    assert!(text.contains("retries=0"));
    assert!(!text.contains(PASSWORD));
}
