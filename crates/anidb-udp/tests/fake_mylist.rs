//! Client tests against the fake `AniDB` server of `common`: the MYLIST, MYLISTADD and
//! MYLISTDEL commands.

mod common;

use common::{exchange, expect_invalid_argument, expect_reply_error, standard_server, ED2K, ENTRY};

use anidb_udp::commands::{
    IdOrName, MylistAddOptions, MylistAdded, MylistAddedByAnime, MylistAnimeSpec, MylistFile,
};
use anidb_udp::mylist::{MylistByAnime, MylistState};
use anidb_udp::{Error, ResponseCode};

// ---- one test per command: the request line sent and the decoded result

#[test]
fn mylist_by_id_commands() {
    let reply = format!("221 MYLIST\n{ENTRY}");
    let line = exchange(&reply, |client| {
        let entry = client.mylist_by_id(5).unwrap();
        assert_eq!((entry.lid, entry.fid, entry.aid), (5, 99, 10_914));
    });
    assert_eq!(line, "MYLIST lid=5");
    let line = exchange(&reply, |client| {
        assert_eq!(client.mylist_by_file_id(99).unwrap().lid, 5);
    });
    assert_eq!(line, "MYLIST fid=99");
    exchange("321 NO SUCH ENTRY", |client| {
        let error = client.mylist_by_file_id(99).unwrap_err();
        expect_reply_error(&error, ResponseCode::NoSuchEntry);
    });
}

#[test]
fn mylist_by_anime_command() {
    let line = exchange(&format!("221 MYLIST\n{ENTRY}"), |client| {
        let MylistByAnime::Single(entry) = client
            .mylist_by_anime(
                IdOrName::Name("Cowboy Bebop"),
                Some(IdOrName::Id(2)),
                Some(3),
            )
            .unwrap()
        else {
            panic!("not a single entry");
        };
        assert_eq!(entry.lid, 5);
    });
    assert_eq!(line, "MYLIST epno=3&aname=Cowboy Bebop&gid=2");
    let line = exchange(
        "312 MULTIPLE MYLIST ENTRIES\nCowboy Bebop|26||1-26|||1-3|GN|1-26",
        |client| {
            let MylistByAnime::Multiple(summary) = client
                .mylist_by_anime(IdOrName::Id(23), None, None)
                .unwrap()
            else {
                panic!("not a summary");
            };
            assert_eq!(summary.anime_title, "Cowboy Bebop");
            assert_eq!(summary.episodes, 26);
            assert_eq!(summary.groups.len(), 1);
            assert_eq!(summary.groups[0].group, "GN");
        },
    );
    assert_eq!(line, "MYLIST aid=23");
}

#[test]
fn mylist_add_by_file_command() {
    let options = MylistAddOptions {
        state: Some(MylistState::InternalStorage),
        viewed: Some(true),
        other: Some("a\nb".into()),
        ..MylistAddOptions::default()
    };
    let line = exchange("210 MYLIST ENTRY ADDED\n5", |client| {
        let added = client
            .mylist_add_by_file(MylistFile::Fid(99), &options)
            .unwrap();
        assert_eq!(added, MylistAdded::Added { lid: 5 });
    });
    assert_eq!(line, "MYLISTADD fid=99&state=1&viewed=1&other=a<br />b");
    let edit = MylistAddOptions {
        edit: Some(true),
        ..MylistAddOptions::default()
    };
    let line = exchange("311 MYLIST ENTRY EDITED\n1", |client| {
        let edited = client
            .mylist_add_by_file(MylistFile::Lid(5), &edit)
            .unwrap();
        assert_eq!(edited, MylistAdded::Edited { edited: 1 });
    });
    assert_eq!(line, "MYLISTADD lid=5&edit=1");
    let line = exchange(&format!("310 FILE ALREADY IN MYLIST\n{ENTRY}"), |client| {
        let error = client
            .mylist_add_by_file(
                MylistFile::Hash {
                    ed2k: ED2K,
                    size: 447_945_964,
                },
                &MylistAddOptions::default(),
            )
            .unwrap_err();
        let Error::FileAlreadyInMylist { entry } = error else {
            panic!("{error:?}");
        };
        assert_eq!(entry.lid, 5);
    });
    assert_eq!(line, format!("MYLISTADD ed2k={ED2K}&size=447945964"));
}

#[test]
fn mylist_add_by_file_rejects_lid_without_edit_before_sending() {
    let server = standard_server();
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    let options = MylistAddOptions {
        edit: Some(false),
        ..MylistAddOptions::default()
    };
    let error = client
        .mylist_add_by_file(MylistFile::Lid(5), &options)
        .unwrap_err();
    expect_invalid_argument(&error);
    drop(client);
    assert_eq!(server.commands(), ["AUTH", "LOGOUT"]);
}

#[test]
fn mylist_add_by_anime_command() {
    let spec = MylistAnimeSpec {
        anime: IdOrName::Id(23),
        group: None,
        epno: -12,
    };
    let line = exchange("210 MYLIST ENTRY ADDED\n12", |client| {
        let added = client
            .mylist_add_by_anime(&spec, &MylistAddOptions::default())
            .unwrap();
        assert_eq!(added, MylistAddedByAnime::Added(12));
    });
    assert_eq!(line, "MYLISTADD aid=23&generic=1&epno=-12");
    let spec = MylistAnimeSpec {
        anime: IdOrName::Name("Cowboy Bebop"),
        group: Some(IdOrName::Name("GN")),
        epno: 0,
    };
    let edit = MylistAddOptions {
        edit: Some(true),
        ..MylistAddOptions::default()
    };
    let line = exchange("311 MYLIST ENTRY EDITED\n3", |client| {
        let edited = client.mylist_add_by_anime(&spec, &edit).unwrap();
        assert_eq!(edited, MylistAddedByAnime::Edited(3));
    });
    assert_eq!(line, "MYLISTADD aname=Cowboy Bebop&gname=GN&epno=0&edit=1");
}

#[test]
fn mylist_del_by_file_command() {
    let line = exchange("211 MYLIST ENTRY DELETED\n2", |client| {
        assert_eq!(client.mylist_del_by_file(MylistFile::Lid(5)).unwrap(), 2);
    });
    assert_eq!(line, "MYLISTDEL lid=5");
    let line = exchange("211 MYLIST ENTRY DELETED\n1", |client| {
        let file = MylistFile::Hash {
            ed2k: ED2K,
            size: 447_945_964,
        };
        assert_eq!(client.mylist_del_by_file(file).unwrap(), 1);
    });
    assert_eq!(line, format!("MYLISTDEL ed2k={ED2K}&size=447945964"));
    exchange("411 NO SUCH MYLIST ENTRY", |client| {
        let error = client.mylist_del_by_file(MylistFile::Fid(1)).unwrap_err();
        expect_reply_error(&error, ResponseCode::NoSuchMylistEntry);
    });
}

#[test]
fn mylist_del_by_anime_command() {
    let line = exchange("211 MYLIST ENTRY DELETED\n4", |client| {
        let removed = client
            .mylist_del_by_anime(IdOrName::Id(23), Some(IdOrName::Id(2)), Some(3))
            .unwrap();
        assert_eq!(removed, 4);
    });
    assert_eq!(line, "MYLISTDEL aid=23&gid=2&epno=3");
    let line = exchange("211 MYLIST ENTRY DELETED\n9", |client| {
        let removed = client
            .mylist_del_by_anime(IdOrName::Name("Cowboy Bebop"), None, None)
            .unwrap();
        assert_eq!(removed, 9);
    });
    assert_eq!(line, "MYLISTDEL aname=Cowboy Bebop");
}

#[test]
fn mylist_stats_command() {
    let line = exchange(
        "222 MYLIST STATS\n1|2|3|4|5|6|7|8|9|10|11|12|13|14|15|16|17",
        |client| {
            let stats = client.mylist_stats().unwrap();
            assert_eq!(stats.anime, 1);
            assert_eq!(stats.views_in_min, 17);
        },
    );
    assert_eq!(line, "MYLISTSTATS ");
}

#[test]
fn mylist_edit_reply_without_a_count_is_one_edit() {
    let edit = MylistAddOptions {
        edit: Some(true),
        ..MylistAddOptions::default()
    };
    // the bare form of 311
    let line = exchange("311 MYLIST ENTRY EDITED", |client| {
        let edited = client
            .mylist_add_by_file(MylistFile::Lid(5), &edit)
            .unwrap();
        assert_eq!(edited, MylistAdded::Edited { edited: 1 });
    });
    assert_eq!(line, "MYLISTADD lid=5&edit=1");
    // the form with the number of entries edited
    exchange("311 MYLIST ENTRY EDITED\n2", |client| {
        let edited = client
            .mylist_add_by_file(MylistFile::Lid(5), &edit)
            .unwrap();
        assert_eq!(edited, MylistAdded::Edited { edited: 2 });
    });
}

#[test]
fn mylist_add_by_anime_bare_311_is_one_edit() {
    let spec = MylistAnimeSpec {
        anime: IdOrName::Id(7),
        group: None,
        epno: 3,
    };
    let edit = MylistAddOptions {
        edit: Some(true),
        ..MylistAddOptions::default()
    };
    exchange("311 MYLIST ENTRY EDITED", |client| {
        let edited = client.mylist_add_by_anime(&spec, &edit).unwrap();
        assert_eq!(edited, MylistAddedByAnime::Edited(1));
    });
    exchange("311 MYLIST ENTRY EDITED\n3", |client| {
        let edited = client.mylist_add_by_anime(&spec, &edit).unwrap();
        assert_eq!(edited, MylistAddedByAnime::Edited(3));
    });
}

#[test]
fn mylist_add_by_anime_request_lines() {
    let anime = IdOrName::Id(7);
    let group = MylistAnimeSpec {
        anime,
        group: Some(IdOrName::Id(2)),
        epno: 3,
    };
    let line = exchange("210 MYLIST ENTRY ADDED\n1", |client| {
        client
            .mylist_add_by_anime(&group, &MylistAddOptions::default())
            .unwrap();
    });
    assert_eq!(line, "MYLISTADD aid=7&gid=2&epno=3");

    // without a group `generic=1` is sent on purpose, so that a file AniDB does
    // not know yet is still listed, as a generic entry
    let generic = MylistAnimeSpec {
        anime,
        group: None,
        epno: 3,
    };
    let line = exchange("210 MYLIST ENTRY ADDED\n1", |client| {
        client
            .mylist_add_by_anime(&generic, &MylistAddOptions::default())
            .unwrap();
    });
    assert_eq!(line, "MYLISTADD aid=7&generic=1&epno=3");

    let edit = MylistAddOptions {
        edit: Some(true),
        ..MylistAddOptions::default()
    };
    let line = exchange("311 MYLIST ENTRY EDITED\n1", |client| {
        client.mylist_add_by_anime(&generic, &edit).unwrap();
    });
    assert_eq!(line, "MYLISTADD aid=7&generic=1&epno=3&edit=1");
}

#[test]
fn mylist_export_commands() {
    let line = exchange("217 EXPORT QUEUED", |client| {
        client.request_mylist_export("xml").unwrap();
    });
    assert_eq!(line, "MYLISTEXPORT template=xml");
    exchange("318 EXPORT ALREADY IN QUEUE", |client| {
        let error = client.request_mylist_export("xml").unwrap_err();
        expect_reply_error(&error, ResponseCode::ExportAlreadyInQueue);
    });
    let line = exchange("218 EXPORT CANCELLED", |client| {
        client.cancel_mylist_export().unwrap();
    });
    assert_eq!(line, "MYLISTEXPORT cancel=1");
}
