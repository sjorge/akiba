use super::*;
use crate::DateFlag;

fn line(text: &str) -> Vec<String> {
    text.split('|').map(str::to_owned).collect()
}

fn lines(text: &str) -> Vec<Vec<String>> {
    text.split('\n').map(line).collect()
}

#[test]
fn anime_desc_keeps_pipes_and_joins_before_line_breaks() {
    let first = AnimeDesc::parse(&lines("1|3|first | part<br ")).unwrap();
    let second = AnimeDesc::parse(&lines("1|3|/>line")).unwrap();
    assert_eq!(first.current_part, 1);
    assert_eq!(first.max_parts, 3);
    assert_eq!(first.desc, "first | part<br ");
    // a `<br />` cut between two parts only becomes a newline after joining
    assert_eq!(second.desc, "/>line");
    let parts = [first, second];
    assert_eq!(join_description(&parts), "first | part\nline");
    assert!(matches!(
        AnimeDesc::parse(&lines("1|3")),
        Err(Error::BadData(_))
    ));
}

#[test]
fn calendar_lines_skip_empty_ones() {
    let entries =
        CalendarEntry::parse_lines(&lines("10914|1300000000|1\n\n10915|1300000500|")).unwrap();
    assert_eq!(
        entries,
        vec![
            CalendarEntry {
                aid: 10_914,
                startdate: 1_300_000_000,
                dateflags: vec![DateFlag::StartUnknownDay],
            },
            CalendarEntry {
                aid: 10_915,
                startdate: 1_300_000_500,
                dateflags: vec![],
            },
        ]
    );
}

#[test]
fn character_with_blocks_and_episode_list() {
    let character = Character::parse(&line(
        "9001|Kanji Name|Transc|pic.jpg|10914,0,123,1'10915,2,,|1,2,3|1600000000|1|f",
    ))
    .unwrap();
    assert_eq!(character.charid, 9001);
    assert_eq!(character.character_name_transc, "Transc");
    assert_eq!(
        character.anime_blocks,
        vec![
            CharacterAnime {
                aid: 10_914,
                appearance: 0,
                creatorid: 123,
                is_main_seiyuu: true,
            },
            CharacterAnime {
                aid: 10_915,
                appearance: 2,
                creatorid: 0,
                is_main_seiyuu: false,
            },
        ]
    );
    assert_eq!(character.episode_list, vec![1, 2, 3]);
    assert_eq!(character.character_type, 1);
    assert_eq!(character.gender, "f");
}

#[test]
fn character_without_seiyuu_or_episodes() {
    let character = Character::parse(&line("9001|K|T|p|10914,0,,|||0|0|")).unwrap();
    assert_eq!(
        character.anime_blocks,
        vec![CharacterAnime {
            aid: 10_914,
            appearance: 0,
            creatorid: 0,
            is_main_seiyuu: false,
        }]
    );
    assert_eq!(character.episode_list, Vec::<i64>::new());
    assert_eq!(character.last_update_date, 0);
    assert_eq!(character.gender, "0");
}

#[test]
fn creator_types_and_unknown_codes() {
    let creator = Creator::parse(&line("77|K|T|1|pic|u1|u2|w1|w2|1600000000")).unwrap();
    assert_eq!(creator.creator_type, Some(CreatorType::Person));
    assert_eq!(creator.last_update_date, 1_600_000_000);
    let unknown = Creator::parse(&line("77|K|T|9|pic|u1|u2|w1|w2|1600000000")).unwrap();
    assert_eq!(unknown.creator_type, None);
}

#[test]
fn episode_types_regular_special_and_unknown() {
    let regular = Episode::parse(&line(
        "163945|10914|24|850|120|1|Episode name|Romaji|Kanji|1300000000|1",
    ))
    .unwrap();
    assert_eq!(regular.episode_type, Some(EpisodeType::Regular));
    assert_eq!(regular.epno, "1");
    assert_eq!(regular.aired, 1_300_000_000);

    let special = Episode::parse(&line("163946|10914|24|0|0|S1|Special|Romaji|Kanji|0|2")).unwrap();
    assert_eq!(special.episode_type, Some(EpisodeType::Special));
    assert_eq!(special.epno, "S1");

    let unknown = Episode::parse(&line("1|2|3|4|5|6|7|8|9|10|9")).unwrap();
    assert_eq!(unknown.episode_type, None);
    assert_eq!(unknown.length, 3);
    assert!(matches!(
        Episode::parse(&line("1|2|x|4|5|6|7|8|9|10|1")),
        Err(Error::BadData(_))
    ));
}

#[test]
fn group_with_relations() {
    let group = Group::parse(&line(
            "13184|850|100|12|10|Group Name|GN|#irc|irc.example|http://g|pic|1200000000|0|1|1300000000|1400000000|5,1'7,105",
        ))
        .unwrap();
    assert_eq!(group.name, "Group Name");
    assert_eq!(group.short, "GN");
    assert_eq!(group.dateflags, vec![DateFlag::StartUnknownDay]);
    assert_eq!(group.lastactivitydate, 1_400_000_000);
    assert_eq!(
        group.grouprelations,
        vec![
            GroupRelation {
                othergroupid: 5,
                relationtype: Some(RelationType::ParticipantIn),
            },
            GroupRelation {
                othergroupid: 7,
                relationtype: Some(RelationType::Formerly),
            },
        ]
    );
}

#[test]
fn group_without_relations_has_none() {
    let group = Group::parse(&line(
            "13184|850|100|12|10|Group Name|GN|#irc|irc.example|http://g|pic|1200000000|0|0|1300000000|1400000000|",
        ))
        .unwrap();
    assert_eq!(group.grouprelations, Vec::<GroupRelation>::new());
    assert_eq!(group.dateflags, vec![]);
}

#[test]
fn group_status_lines_and_states() {
    let entries = GroupStatusEntry::parse_lines(&lines(
        "13184|Group|1|12|850|100|1-12\n99|Other|6|2|0|0|S1\n",
    ))
    .unwrap();
    assert_eq!(
        entries,
        vec![
            GroupStatusEntry {
                gid: 13_184,
                group_name: "Group".into(),
                completion_state: Some(CompletionState::Ongoing),
                last_episode_number: 12,
                rating: 850,
                votes: 100,
                episode_range: "1-12".into(),
            },
            GroupStatusEntry {
                gid: 99,
                group_name: "Other".into(),
                completion_state: Some(CompletionState::SpecialsOnly),
                last_episode_number: 2,
                rating: 0,
                votes: 0,
                episode_range: "S1".into(),
            },
        ]
    );
    let unknown = GroupStatusEntry::parse(&line("13184|Group|9|12|850|100|1-12")).unwrap();
    assert_eq!(unknown.completion_state, None);
}

#[test]
fn updated_lists_and_no_updates() {
    let updated = Updated::parse(&line("1|55|1600000000|10914,10915")).unwrap();
    assert_eq!(updated.total_count, 55);
    assert_eq!(updated.last_update_date, 1_600_000_000);
    assert_eq!(updated.updated_aids, vec![10_914, 10_915]);
    let empty = Updated::parse(&line("1|0|0|")).unwrap();
    assert_eq!(empty.updated_aids, Vec::<i64>::new());
    assert_eq!(
        Updated::no_updates(),
        Updated {
            total_count: 0,
            last_update_date: 0,
            updated_aids: vec![],
        }
    );
    assert!(matches!(
        Updated::parse(&line("1|x|0|")),
        Err(Error::BadData(_))
    ));
}

#[test]
fn mylist_stats_in_reply_order() {
    let stats = MylistStats::parse(&line("1|2|3|4|5|6|7|8|9|10|11|12|13|14|15|16|17")).unwrap();
    assert_eq!(stats.anime, 1);
    assert_eq!(stats.size_of_files, 4);
    assert_eq!(stats.leech_perc, 9);
    assert_eq!(stats.mylist_perc, 12);
    assert_eq!(stats.reviews, 16);
    assert_eq!(stats.views_in_min, 17);
    assert!(matches!(
        MylistStats::parse(&line("1|2|3")),
        Err(Error::BadData(_))
    ));
}

#[test]
fn vote_actions_and_records() {
    let voted = VoteResult::parse(ResponseCode::Voted, &line("Anime Name|500|1|10914")).unwrap();
    assert_eq!(voted.action, VoteAction::Voted);
    assert_eq!(
        voted.vote,
        VoteRecord {
            name: "Anime Name".into(),
            vote_value: 500,
            vote_type: Some(VoteType::Anime),
            id: 10_914,
        }
    );
    let revoked = VoteResult::parse(ResponseCode::VoteRevoked, &line("Ep|-1|6|163945")).unwrap();
    assert_eq!(revoked.action, VoteAction::VoteRevoked);
    assert_eq!(revoked.vote.vote_type, Some(VoteType::Episode));
    assert_eq!(revoked.vote.vote_value, -1);

    let unknown = VoteRecord::parse(&line("Ep|500|4|163945")).unwrap();
    assert_eq!(unknown.vote_type, None);

    assert!(matches!(
        VoteResult::parse(ResponseCode::NoSuchVote, &line("Ep|500|1|1")),
        Err(Error::BadData(_))
    ));
}

#[test]
fn random_anime_has_the_19_anime_fields() {
    let reply = "10914|26|26|2|850|120|845|30|870|4|1998|TV|R|K|E|o1'o2|s1|syn|1,2";
    let random = RandomAnime::parse(&line(reply)).unwrap();
    assert_eq!(random.aid, 10_914);
    assert_eq!(random.anime_type, "TV");
    assert_eq!(random.other_name_list, vec!["o1", "o2"]);
    assert_eq!(random.short_name_list, vec!["s1"]);
    assert_eq!(random.synonym_list, vec!["syn"]);
    assert_eq!(random.categories, vec!["1", "2"]);
    // extra trailing fields are ignored
    assert_eq!(
        RandomAnime::parse(&line(&format!("{reply}|900000000|0|5|1|pic.jpg"))).unwrap(),
        random
    );
    assert!(matches!(
        RandomAnime::parse(&line("10914|26")),
        Err(Error::BadData(_))
    ));
    assert!(matches!(
        RandomAnime::parse(&line(&reply[..reply.rfind('|').unwrap()])),
        Err(Error::BadData(_))
    ));
}

#[test]
fn text_fields_decode_line_breaks() {
    let random = RandomAnime::parse(&line(
        "1|2|2|0|0|0|0|0|0|0|1998|TV|a<br />b|K|E|o<br />1'o2|s|syn|1",
    ))
    .unwrap();
    assert_eq!(random.romanji_name, "a\nb");
    assert_eq!(random.other_name_list, vec!["o\n1", "o2"]);
    let episode = Episode::parse(&line("1|2|24|850|10|1|Eng<br />lish|Rom|Kan|0|1")).unwrap();
    assert_eq!(episode.eng, "Eng\nlish");
}

#[test]
fn user_reply() {
    let user = UserId::parse(&line("42|some user")).unwrap();
    assert_eq!(user.uid, 42);
    assert_eq!(user.username, "some user");
    assert!(matches!(
        UserId::parse(&line("4x2|some user")),
        Err(Error::BadData(_))
    ));
}
