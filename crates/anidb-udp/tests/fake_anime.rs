//! Client tests against the fake `AniDB` server of `common`: the ANIME command and the
//! other info commands.

mod common;

use common::{exchange, expect_reply_error, standard, FakeServer, Out};

use anidb_udp::anime::{self, AnimeCommandField};
use anidb_udp::commands::{IdOrName, UpdatedSince};
use anidb_udp::mask::Value;
use anidb_udp::responses::{CalendarEntry, CompletionState, CreatorType, EpisodeType};
use anidb_udp::ResponseCode;

// ---- one test per command: the request line sent and the decoded result

#[test]
fn anime_command() {
    let fields = anime::mask([AnimeCommandField::Year]);
    let line = exchange("230 ANIME\n10914|1998", |client| {
        let reply = client.anime(IdOrName::Id(10_914), Some(&fields)).unwrap();
        assert_eq!(reply.aid, 10_914);
        assert_eq!(
            reply.fields[&AnimeCommandField::Year],
            Value::Str("1998".into())
        );
    });
    assert_eq!(line, "ANIME amask=a0000000000000&aid=10914");
    let line = exchange("230 ANIME\n10914|1998", |client| {
        client
            .anime(IdOrName::Name("Cowboy Bebop"), Some(&fields))
            .unwrap();
    });
    assert_eq!(line, "ANIME amask=a0000000000000&aname=Cowboy Bebop");
    let line = exchange("330 NO SUCH ANIME", |client| {
        let error = client.anime(IdOrName::Id(1), None).unwrap_err();
        expect_reply_error(&error, ResponseCode::NoSuchAnime);
    });
    assert!(line.starts_with("ANIME amask="));
}

#[test]
fn anime_desc_command_joins_the_parts() {
    let server = FakeServer::spawn(
        None,
        Box::new(
            |request| match (request.command.as_str(), request.param("part")) {
                ("ANIMEDESC", Some("0")) => {
                    vec![Out::Tagged(
                        "233 ANIME DESCRIPTION\n0|2|first<br".to_owned(),
                    )]
                }
                ("ANIMEDESC", Some("1")) => {
                    vec![Out::Tagged(
                        "233 ANIME DESCRIPTION\n1|2| />second".to_owned(),
                    )]
                }
                _ => standard(request),
            },
        ),
    );
    let mut client = server.fast_client();
    client.connect("me", "pw", None).unwrap();
    assert_eq!(client.anime_desc(10_914).unwrap(), "first\nsecond");
    drop(client);
    let lines: Vec<String> = server.requests().iter().map(|r| r.line.clone()).collect();
    assert_eq!(lines[1], "ANIMEDESC aid=10914&part=0");
    assert_eq!(lines[2], "ANIMEDESC aid=10914&part=1");
}

#[test]
fn calendar_command() {
    let line = exchange(
        "297 CALENDAR\n10914|1300000000|1\n10915|1300000500|",
        |client| {
            let entries = client.calendar().unwrap();
            assert_eq!(entries.len(), 2);
            assert_eq!(
                entries[1],
                CalendarEntry {
                    aid: 10_915,
                    startdate: 1_300_000_500,
                    dateflags: vec![],
                }
            );
        },
    );
    assert_eq!(line, "CALENDAR ");
    exchange("397 CALENDAR EMPTY", |client| {
        assert_eq!(client.calendar().unwrap().len(), 0);
    });
}

#[test]
fn character_command() {
    let line = exchange(
        "235 CHARACTER\n9001|Kanji|Transc|pic.jpg|10914,0,123,1'10915,2,,|1,2,3|1600000000|1|f",
        |client| {
            let character = client.character(9001).unwrap();
            assert_eq!(character.charid, 9001);
            assert_eq!(character.anime_blocks.len(), 2);
            assert_eq!(character.episode_list, vec![1, 2, 3]);
        },
    );
    assert_eq!(line, "CHARACTER charid=9001");
}

#[test]
fn creator_command() {
    let line = exchange(
        "245 CREATOR\n77|K|T|1|pic|u1|u2|w1|w2|1600000000",
        |client| {
            let creator = client.creator(77).unwrap();
            assert_eq!(creator.creator_type, Some(CreatorType::Person));
        },
    );
    assert_eq!(line, "CREATOR creatorid=77");
}

#[test]
fn episode_commands() {
    let reply = "240 EPISODE\n163945|10914|24|850|120|S1|Episode name|Romaji|Kanji|1300000000|2";
    let line = exchange(reply, |client| {
        let episode = client.episode(163_945).unwrap();
        assert_eq!(episode.epno, "S1");
        assert_eq!(episode.episode_type, Some(EpisodeType::Special));
    });
    assert_eq!(line, "EPISODE eid=163945");
    let line = exchange(reply, |client| {
        let episode = client
            .episode_by_anime(IdOrName::Name("Cowboy Bebop"), "S1")
            .unwrap();
        assert_eq!(episode.eid, 163_945);
    });
    assert_eq!(line, "EPISODE epno=S1&aname=Cowboy Bebop");
    let line = exchange(reply, |client| {
        client.episode_by_anime(IdOrName::Id(23), "S1").unwrap();
    });
    assert_eq!(line, "EPISODE epno=S1&aid=23");
}

#[test]
fn group_command() {
    let reply = "250 GROUP\n13184|850|100|12|10|Group Name|GN|#irc|irc.example|http://g|pic|1200000000|0|0|1300000000|1400000000|";
    let line = exchange(reply, |client| {
        let group = client.group(IdOrName::Id(13_184)).unwrap();
        assert_eq!(group.name, "Group Name");
        assert_eq!(group.grouprelations.len(), 0);
    });
    assert_eq!(line, "GROUP gid=13184");
    let line = exchange(reply, |client| {
        client.group(IdOrName::Name("GN")).unwrap();
    });
    assert_eq!(line, "GROUP gname=GN");
}

#[test]
fn groupstatus_command() {
    let reply = "225 GROUP STATUS\n13184|Group|1|12|850|100|1-12\n99|Other|6|2|0|0|S1";
    let line = exchange(reply, |client| {
        let entries = client.groupstatus(23, None).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[1].completion_state,
            Some(CompletionState::SpecialsOnly)
        );
    });
    assert_eq!(line, "GROUPSTATUS aid=23");
    let line = exchange(reply, |client| {
        client
            .groupstatus(23, Some(CompletionState::Finished))
            .unwrap();
    });
    assert_eq!(line, "GROUPSTATUS aid=23&state=5");
    exchange("325 NO GROUPS FOUND", |client| {
        let error = client.groupstatus(23, None).unwrap_err();
        expect_reply_error(&error, ResponseCode::NoGroupsFound);
    });
}

#[test]
fn updated_command() {
    let line = exchange("243 UPDATED\n1|55|1600000000|10914,10915", |client| {
        let updated = client.updated(UpdatedSince::Age(3)).unwrap();
        assert_eq!(updated.total_count, 55);
        assert_eq!(updated.updated_aids, vec![10_914, 10_915]);
    });
    assert_eq!(line, "UPDATED entity=1&age=3");
    let line = exchange("343 NO UPDATES", |client| {
        let updated = client.updated(UpdatedSince::Time(1_600_000_000)).unwrap();
        assert_eq!(updated.total_count, 0);
        assert_eq!(updated.updated_aids.len(), 0);
    });
    assert_eq!(line, "UPDATED entity=1&time=1600000000");
}
