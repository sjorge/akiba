//! Client tests against the fake `AniDB` server of `common`: the commands that are not
//! about files, anime or the mylist.

mod common;

use common::{exchange, expect_invalid_argument, expect_reply_error, standard_server};

use anidb_udp::commands::{Encoding, IdOrName, RandomType, VoteChoice};
use anidb_udp::responses::{VoteAction, VoteType};
use anidb_udp::{Error, ResponseCode};

// ---- one test per command: the request line sent and the decoded result

#[test]
fn encoding_command() {
    let line = exchange("219 ENCODING CHANGED", |client| {
        client.encoding(Encoding::Utf8).unwrap();
    });
    assert_eq!(line, "ENCODING name=UTF8");
    let line = exchange("519 ENCODING NOT SUPPORTED", |client| {
        let error = client.encoding(Encoding::Utf16).unwrap_err();
        expect_reply_error(&error, ResponseCode::EncodingNotSupported);
    });
    assert_eq!(line, "ENCODING name=UTF-16");
}

#[test]
fn version_command() {
    let line = exchange("998 VERSION\nAniDB UDP 1.2", |client| {
        assert_eq!(client.version().unwrap(), "AniDB UDP 1.2");
    });
    assert_eq!(line, "VERSION ");
}

#[test]
fn uptime_command() {
    let line = exchange("208 UPTIME\n123456", |client| {
        assert_eq!(client.uptime().unwrap(), 123_456);
    });
    assert_eq!(line, "UPTIME ");
}

#[test]
fn vote_command() {
    let line = exchange("261 VOTE FOUND\nCowboy Bebop|850|1|23", |client| {
        let result = client
            .vote(VoteType::Anime, IdOrName::Id(23), VoteChoice::Get, None)
            .unwrap();
        assert_eq!(result.action, VoteAction::VoteFound);
        assert_eq!(result.vote.vote_value, 850);
        assert_eq!(result.vote.id, 23);
    });
    assert_eq!(line, "VOTE type=1&id=23&value=0");
    let line = exchange("263 VOTE REVOKED\nEp 2|-1|1|23", |client| {
        let result = client
            .vote(
                VoteType::Anime,
                IdOrName::Name("Cowboy Bebop"),
                VoteChoice::Revoke,
                Some("2"),
            )
            .unwrap();
        assert_eq!(result.action, VoteAction::VoteRevoked);
    });
    assert_eq!(line, "VOTE type=1&name=Cowboy Bebop&epno=2&value=-1");
    let line = exchange(
        "363 PERMVOTE NOT ALLOWED\nCowboy Bebop|850|1|23",
        |client| {
            let error = client
                .vote(
                    VoteType::AnimeTmp,
                    IdOrName::Id(23),
                    VoteChoice::Value(500),
                    None,
                )
                .unwrap_err();
            let Error::VoteRefused { code, vote, .. } = error else {
                panic!("{error:?}");
            };
            assert_eq!(code, ResponseCode::PermvoteNotAllowed);
            assert_eq!(vote.vote_value, 850);
        },
    );
    assert_eq!(line, "VOTE type=2&id=23&value=500");
}

#[test]
fn vote_rejects_bad_input_before_sending() {
    let server = standard_server();
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    let error = client
        .vote(
            VoteType::Group,
            IdOrName::Id(1),
            VoteChoice::Value(99),
            None,
        )
        .unwrap_err();
    expect_invalid_argument(&error);
    let error = client
        .vote(VoteType::Group, IdOrName::Id(1), VoteChoice::Get, Some("1"))
        .unwrap_err();
    expect_invalid_argument(&error);
    drop(client);
    assert_eq!(server.commands(), ["AUTH", "LOGOUT"]);
}

#[test]
fn random_command() {
    let line = exchange(
        "230 ANIME\n10914|26|26|2|850|120|845|30|870|4|1998|TV|R|K|E|o1'o2|s1|syn|1,2",
        |client| {
            let random = client.random(RandomType::Watched).unwrap();
            assert_eq!(random.aid, 10_914);
            assert_eq!(random.categories, ["1", "2"]);
        },
    );
    assert_eq!(line, "RANDOMANIME type=1");
}

#[test]
fn send_message_command() {
    let line = exchange("294 SENDMESSAGE SUCCESSFUL", |client| {
        client
            .send_message("bob", "hello", "line one\nline two")
            .unwrap();
    });
    assert_eq!(
        line,
        "SENDMSG to=bob&title=hello&body=line one<br />line two"
    );
    exchange("394 NO SUCH USER", |client| {
        let error = client.send_message("nobody", "t", "b").unwrap_err();
        expect_reply_error(&error, ResponseCode::NoSuchUser);
    });
}

#[test]
fn send_message_rejects_bad_input_before_sending() {
    let server = standard_server();
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    for (user, title, body) in [
        ("", "t", "b"),
        ("u", "", "b"),
        ("u", "t", ""),
        ("u", &"t".repeat(51), "b"),
        ("u", "t", &"b".repeat(901)),
    ] {
        let error = client.send_message(user, title, body).unwrap_err();
        expect_invalid_argument(&error);
    }
    drop(client);
    assert_eq!(server.commands(), ["AUTH", "LOGOUT"]);
}

#[test]
fn get_user_id_command() {
    let line = exchange("295 USER ID\n42|some user", |client| {
        let user = client.get_user_id(IdOrName::Name("some user")).unwrap();
        assert_eq!((user.uid, user.username.as_str()), (42, "some user"));
    });
    assert_eq!(line, "USER user=some user");
    let line = exchange("295 USER ID\n42|some user", |client| {
        assert_eq!(client.get_user_id(IdOrName::Id(42)).unwrap().uid, 42);
    });
    assert_eq!(line, "USER uid=42");
}
