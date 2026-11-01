use super::*;

#[test]
fn encodes_params_in_order_with_tag_and_session_last() {
    let request = Request::new("AUTH")
        .param("user", "x")
        .param("pass", "a&b\nc")
        .param("protover", "3");
    assert_eq!(
        request.encode(Some("sess"), "ABCDE"),
        "AUTH user=x&pass=a&amp;b<br />c&protover=3&tag=ABCDE&s=sess"
    );
}

// expected string computed independently with the same input
#[test]
fn encoding_matches_the_reference_string() {
    let request = Request::new("AUTH")
        .param("user", "x")
        .param("pass", "a&b\nc\r\nd\re")
        .param("protover", "3");
    assert_eq!(
        request.encode(None, "ABCDE"),
        "AUTH user=x&pass=a&amp;b<br />c<br />d<br />e&protover=3&tag=ABCDE"
    );
}

#[test]
fn encodes_without_session() {
    let request = Request::new("PING");
    assert_eq!(request.encode(None, "ABCDE"), "PING tag=ABCDE");
}

#[test]
fn escapes_every_newline_style_once() {
    let request = Request::new("X").param("v", "a\r\nb\rc\nd");
    assert_eq!(
        request.encode(None, "T"),
        "X v=a<br />b<br />c<br />d&tag=T"
    );
}

#[test]
fn parses_single_line_tagged_reply() {
    let response = parse("ABCDE 200 sess LOGIN ACCEPTED").unwrap();
    assert_eq!(
        response,
        Response {
            tag: Some("ABCDE".into()),
            code: ResponseCode::LoginAccepted,
            message: "sess LOGIN ACCEPTED".into(),
            lines: Vec::new(),
        }
    );
}

#[test]
fn parses_multi_line_reply() {
    let response = parse("ABCDE 221 MYLIST\n1|2|3\n4|5|6\n").unwrap();
    assert_eq!(response.tag.as_deref(), Some("ABCDE"));
    assert_eq!(response.code, ResponseCode::Mylist);
    assert_eq!(response.message, "MYLIST");
    assert_eq!(
        response.lines,
        vec![
            vec!["1".to_owned(), "2".into(), "3".into()],
            vec!["4".to_owned(), "5".into(), "6".into()],
        ]
    );
}

#[test]
fn keeps_empty_fields() {
    let response = parse("ABCDE 221 X\n|a|").unwrap();
    assert_eq!(response.lines, vec![vec!["", "a", ""]]);
}

#[test]
fn parses_untagged_errors() {
    let banned = parse("555 BANNED\nreason|x").unwrap();
    assert_eq!(banned.tag, None);
    assert_eq!(banned.code, ResponseCode::Banned);
    assert_eq!(banned.message, "BANNED");
    assert_eq!(banned.lines, vec![vec!["reason", "x"]]);

    let out_of_service = parse("601 ANIDB OUT OF SERVICE - PLEASE TRY AGAIN LATER").unwrap();
    assert_eq!(out_of_service.tag, None);
    assert_eq!(out_of_service.code, ResponseCode::AnidbOutOfService);
    assert_eq!(out_of_service.lines, Vec::<Vec<String>>::new());

    let client_banned = parse("504 CLIENT BANNED").unwrap();
    assert_eq!(client_banned.code, ResponseCode::ClientBanned);
}

#[test]
fn parses_tagged_failure_and_trims_trailing_newlines() {
    let response = parse("ABCDE 320 NO SUCH FILE\n\n").unwrap();
    assert_eq!(response.code, ResponseCode::NoSuchFile);
    assert_eq!(response.message, "NO SUCH FILE");
    assert_eq!(response.lines, Vec::<Vec<String>>::new());
}

#[test]
fn rejects_text_without_a_code() {
    for text in ["", "hello", "AB 200 x", "ABCDE 2x0 y", "200", "ABCDE 20"] {
        assert!(
            matches!(parse(text), Err(Error::BadResponse(_))),
            "accepted {text:?}"
        );
    }
}

#[test]
fn tags_are_five_letters_or_digits() {
    for _ in 0..200 {
        let tag = make_tag();
        assert_eq!(tag.len(), TAG_LENGTH);
        assert!(tag.bytes().all(|b| b.is_ascii_alphanumeric()), "{tag}");
    }
    assert_ne!(make_tag(), make_tag());
}
