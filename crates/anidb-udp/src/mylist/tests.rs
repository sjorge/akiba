use super::*;

fn line(text: &str) -> Vec<String> {
    text.split('|').map(str::to_owned).collect()
}

#[test]
fn parses_an_entry_with_line_breaks() {
    let entry = MylistEntry::parse(&line(
            "212018403|1628637|163945|10914|13184|1400000000|1|1400000100|shelf 2|ed2k|line one<br />line two|0",
        ))
        .unwrap();
    assert_eq!(entry.lid, 212_018_403);
    assert_eq!(entry.fid, 1_628_637);
    assert_eq!(entry.aid, 10_914);
    assert_eq!(entry.date, 1_400_000_000);
    assert_eq!(entry.state, Some(MylistState::InternalStorage));
    assert_eq!(entry.viewdate, 1_400_000_100);
    assert_eq!(entry.storage, "shelf 2");
    assert_eq!(entry.source, "ed2k");
    assert_eq!(entry.other, "line one\nline two");
    assert_eq!(entry.filestate, Some(MylistFileState::NormalOriginal));
}

#[test]
fn unknown_or_empty_codes_are_none() {
    let entry = MylistEntry::parse(&line("1|2|3|4|5|6|9|||||15")).unwrap();
    assert_eq!(entry.state, None);
    assert_eq!(entry.filestate, Some(MylistFileState::Streamed));
    assert_eq!(entry.viewdate, 0);

    let entry = MylistEntry::parse(&line("1|2|3|4|5|6||||||101")).unwrap();
    assert_eq!(entry.state, None);
    assert_eq!(entry.filestate, None);
}

#[test]
fn rejects_short_entries() {
    assert!(matches!(
        MylistEntry::parse(&line("1|2|3")),
        Err(Error::BadData(_))
    ));
}

#[test]
fn multiple_entries_summary_lists_each_group() {
    let summary =
        MylistSummary::parse(&line("Trinity Seven|13|2|5|3|1|4|Grp A|1-12|Grp B|1-3,T1")).unwrap();
    assert_eq!(summary.anime_title, "Trinity Seven");
    assert_eq!(summary.episodes, 13);
    assert_eq!(summary.eps_unknown, "2");
    assert_eq!(summary.eps_hdd, "5");
    assert_eq!(summary.eps_cd, "3");
    assert_eq!(summary.eps_deleted, "1");
    assert_eq!(summary.eps_watched, "4");
    assert_eq!(
        summary.groups,
        vec![
            MylistGroup {
                group: "Grp A".into(),
                episodes: "1-12".into(),
            },
            MylistGroup {
                group: "Grp B".into(),
                episodes: "1-3,T1".into(),
            },
        ]
    );
}

#[test]
fn summary_needs_its_head_and_paired_groups() {
    assert!(matches!(
        MylistSummary::parse(&line("Title|13|2|5|3|1")),
        Err(Error::BadData(_))
    ));
    assert!(matches!(
        MylistSummary::parse(&line("Title|13|2|5|3|1|4|Grp A")),
        Err(Error::BadData(_))
    ));
    let no_groups = MylistSummary::parse(&line("Title|1|0|0|0|0|0")).unwrap();
    assert_eq!(no_groups.groups, Vec::<MylistGroup>::new());
}

#[test]
fn by_anime_reply_depends_on_its_code() {
    let single = MylistByAnime::parse(
        ResponseCode::Mylist,
        &line("1|2|3|4|5|6|1||storage|ed2k|note<br />two|0"),
    )
    .unwrap();
    assert!(matches!(single, MylistByAnime::Single(ref entry) if entry.aid == 4));

    let multiple = MylistByAnime::parse(
        ResponseCode::MultipleMylistEntries,
        &line("Title|13|2|5|3|1|4|Grp A|1-12"),
    )
    .unwrap();
    assert!(matches!(multiple, MylistByAnime::Multiple(ref s) if s.groups.len() == 1));

    assert!(matches!(
        MylistByAnime::parse(ResponseCode::NoSuchEntry, &line("")),
        Err(Error::BadData(_))
    ));
}
