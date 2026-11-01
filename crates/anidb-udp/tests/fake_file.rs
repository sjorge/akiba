//! Client tests against the fake `AniDB` server of `common`: the FILE command.

mod common;

use common::{exchange, file_fields, standard, FakeServer, Out, ED2K};

use anidb_udp::commands::IdOrName;
use anidb_udp::mask::{AnimeField, AnimeFields, FileField, FileFields, Value};
use anidb_udp::{Error, MylistAdd, ResponseCode};

#[test]
fn file_already_in_mylist_carries_the_entry() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "MYLISTADD" => vec![Out::Tagged(
                "310 FILE ALREADY IN MYLIST\n5|99|7|10914|3|1700000000|1|0|||ed2k|2".to_owned(),
            )],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    let error = client
        .mylist_add_by_hash(1, ED2K, MylistAdd::default())
        .unwrap_err();
    let Error::FileAlreadyInMylist { entry } = error else {
        panic!("{error:?}");
    };
    assert_eq!((entry.lid, entry.aid), (5, 10_914));
}

#[test]
fn unknown_file_is_reply_320() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "FILE" => vec![Out::Tagged("320 NO SUCH FILE".to_owned())],
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
                code: ResponseCode::NoSuchFile,
                ..
            }
        ),
        "{error:?}"
    );
    // not fatal
    client.ping().unwrap();
}

// ---- one test per command: the request line sent and the decoded result

#[test]
fn file_command() {
    let (file, anime) = file_fields();
    let line = exchange("220 FILE\n99|10914|447945964|13", |client| {
        let (file, anime) = file_fields();
        let reply = client.file(99, &file, &anime).unwrap();
        assert_eq!(reply.fid, 99);
        assert_eq!(reply.file[&FileField::Size], Value::Int(447_945_964));
    });
    assert_eq!(
        line,
        format!(
            "FILE fid=99&fmask={}&amask={}",
            file.to_hex(),
            anime.to_hex()
        )
    );
}

#[test]
fn file_by_episode_command() {
    let (file, anime) = file_fields();
    let line = exchange("220 FILE\n99|10914|447945964|13", |client| {
        let (file, anime) = file_fields();
        let reply = client
            .file_by_episode(
                IdOrName::Name("Cowboy Bebop"),
                IdOrName::Id(13_184),
                "1",
                &file,
                &anime,
            )
            .unwrap();
        assert_eq!(reply.fid, 99);
    });
    assert_eq!(
        line,
        format!(
            "FILE epno=1&fmask={}&amask={}&aname=Cowboy Bebop&gid=13184",
            file.to_hex(),
            anime.to_hex()
        )
    );
    exchange("322 MULTIPLE FILES FOUND\n1628637|1628638", |client| {
        let (file, anime) = file_fields();
        let error = client
            .file_by_episode(IdOrName::Id(23), IdOrName::Name("GN"), "1", &file, &anime)
            .unwrap_err();
        let Error::MultipleFilesFound { fids } = error else {
            panic!("{error:?}");
        };
        assert_eq!(fids, vec![1_628_637, 1_628_638]);
    });
}

#[test]
fn line_breaks_are_decoded_in_file_replies() {
    let server = FakeServer::spawn(
        None,
        Box::new(|request| match request.command.as_str() {
            "FILE" => vec![Out::Tagged(
                "220 FILE\n99|first<br />second|Name<br />Two".to_owned(),
            )],
            _ => standard(request),
        }),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    let file: FileFields = [FileField::Description].into_iter().collect();
    let anime: AnimeFields = [AnimeField::RomanjiName].into_iter().collect();
    let reply = client.file_by_hash(1, ED2K, &file, &anime).unwrap();
    assert_eq!(
        reply.file[&FileField::Description],
        Value::Str("first\nsecond".into())
    );
    assert_eq!(
        reply.anime[&AnimeField::RomanjiName],
        Value::Str("Name\nTwo".into())
    );
}
