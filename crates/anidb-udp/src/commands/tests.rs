use super::*;
use crate::mask::{AnimeField, FileField};

fn line(request: &Request) -> String {
    request.encode(None, "T").replace("&tag=T", "")
}

fn id(value: i64) -> IdOrName<'static> {
    IdOrName::Id(value)
}

fn is_illegal(error: &Error) -> bool {
    matches!(error, Error::InvalidArgument(_))
}

#[test]
fn anime_request_has_the_amask_first() {
    let fields = anime::mask([AnimeCommandField::Year]);
    assert_eq!(
        line(&anime_request(id(1), fields)),
        format!("ANIME amask={}&aid=1", fields.to_hex())
    );
    assert_eq!(
        line(&anime_request(IdOrName::Name("Bebop"), fields)),
        format!("ANIME amask={}&aname=Bebop", fields.to_hex())
    );
}

#[test]
fn file_request_puts_epno_before_the_masks_and_ids_after() {
    let file: FileFields = [FileField::Size].into_iter().collect();
    let anime: AnimeFields = [AnimeField::Year].into_iter().collect();
    let request = file_request(Request::new("FILE").param("epno", "S1"), file, anime);
    let request = target(request, IdOrName::Name("Bebop"), "aid", "aname");
    let request = target(request, id(5), "gid", "gname");
    assert_eq!(
        line(&request),
        format!(
            "FILE epno=S1&fmask={}&amask={}&aname=Bebop&gid=5",
            file.to_hex(),
            anime.to_hex()
        )
    );
}

#[test]
fn mylist_add_by_file_requests() {
    let options = MylistAddOptions {
        edit: Some(true),
        state: Some(MylistState::InternalStorage),
        viewed: Some(false),
        viewdate: Some(7),
        source: Some("ed2k".into()),
        storage: Some("hdd".into()),
        other: Some("a\nb".into()),
    };
    assert_eq!(
        line(&mylist_add_file_request(MylistFile::Fid(9), &options).unwrap()),
        "MYLISTADD fid=9&edit=1&state=1&viewed=0&viewdate=7&source=ed2k&storage=hdd&other=a<br />b"
    );
    assert_eq!(
        line(
            &mylist_add_file_request(
                MylistFile::Hash {
                    ed2k: "abc",
                    size: 5
                },
                &MylistAddOptions::default()
            )
            .unwrap()
        ),
        "MYLISTADD ed2k=abc&size=5"
    );
    // by lid `edit` is implied and sent once
    let edit = MylistAddOptions {
        edit: Some(true),
        viewed: Some(true),
        ..MylistAddOptions::default()
    };
    assert_eq!(
        line(&mylist_add_file_request(MylistFile::Lid(3), &edit).unwrap()),
        "MYLISTADD lid=3&edit=1&viewed=1"
    );
    assert_eq!(
        line(&mylist_add_file_request(MylistFile::Lid(3), &MylistAddOptions::default()).unwrap()),
        "MYLISTADD lid=3&edit=1"
    );
    let refused = MylistAddOptions {
        edit: Some(false),
        ..MylistAddOptions::default()
    };
    assert!(is_illegal(
        &mylist_add_file_request(MylistFile::Lid(3), &refused).unwrap_err()
    ));
    // `edit` false is sent for the other targets
    assert_eq!(
        line(&mylist_add_file_request(MylistFile::Fid(3), &refused).unwrap()),
        "MYLISTADD fid=3&edit=0"
    );
}

#[test]
fn mylist_add_by_anime_requests() {
    let spec = MylistAnimeSpec {
        anime: id(7),
        group: None,
        epno: -12,
    };
    assert_eq!(
        line(&mylist_add_anime_request(&spec, &MylistAddOptions::default()).unwrap()),
        "MYLISTADD aid=7&generic=1&epno=-12"
    );
    let spec = MylistAnimeSpec {
        anime: IdOrName::Name("Bebop"),
        group: Some(IdOrName::Name("grp")),
        epno: 0,
    };
    assert_eq!(
        line(&mylist_add_anime_request(&spec, &MylistAddOptions::default()).unwrap()),
        "MYLISTADD aname=Bebop&gname=grp&epno=0"
    );
    // an edit without a group keeps `generic=1` as well (deliberate)
    let spec = MylistAnimeSpec {
        anime: id(7),
        group: None,
        epno: 3,
    };
    let edit = MylistAddOptions {
        edit: Some(true),
        ..MylistAddOptions::default()
    };
    assert_eq!(
        line(&mylist_add_anime_request(&spec, &edit).unwrap()),
        "MYLISTADD aid=7&generic=1&epno=3&edit=1"
    );
    let empty = MylistAnimeSpec {
        anime: IdOrName::Name(""),
        group: None,
        epno: 0,
    };
    assert!(is_illegal(
        &mylist_add_anime_request(&empty, &MylistAddOptions::default()).unwrap_err()
    ));
}

#[test]
fn mylist_del_requests() {
    assert_eq!(
        line(&mylist_del_anime_request(id(7), Some(id(2)), Some(3)).unwrap()),
        "MYLISTDEL aid=7&gid=2&epno=3"
    );
    assert_eq!(
        line(&mylist_del_anime_request(IdOrName::Name("x"), None, None).unwrap()),
        "MYLISTDEL aname=x"
    );
    assert!(is_illegal(
        &mylist_del_anime_request(id(0), None, None).unwrap_err()
    ));
    assert_eq!(
        line(&file_spec(
            Request::new("MYLISTDEL"),
            MylistFile::Hash { ed2k: "h", size: 1 }
        )),
        "MYLISTDEL ed2k=h&size=1"
    );
}

#[test]
fn vote_requests_and_validation() {
    assert_eq!(
        line(&vote_request(VoteType::Anime, id(1), VoteChoice::Get, Some("2")).unwrap()),
        "VOTE type=1&id=1&epno=2&value=0"
    );
    assert_eq!(
        line(
            &vote_request(
                VoteType::Episode,
                IdOrName::Name("n"),
                VoteChoice::Revoke,
                None
            )
            .unwrap()
        ),
        "VOTE type=6&name=n&value=-1"
    );
    assert_eq!(
        line(&vote_request(VoteType::Group, id(4), VoteChoice::Value(1000), None).unwrap()),
        "VOTE type=3&id=4&value=1000"
    );
    for bad in [0, 99, 1001] {
        assert!(is_illegal(
            &vote_request(VoteType::Anime, id(1), VoteChoice::Value(bad), None).unwrap_err()
        ));
    }
    assert!(is_illegal(
        &vote_request(VoteType::Group, id(1), VoteChoice::Get, Some("1")).unwrap_err()
    ));
}

#[test]
fn send_message_validation() {
    assert_eq!(
        line(&send_message_request("u", "t", "b").unwrap()),
        "SENDMSG to=u&title=t&body=b"
    );
    assert!(is_illegal(&send_message_request("", "t", "b").unwrap_err()));
    assert!(is_illegal(&send_message_request("u", "", "b").unwrap_err()));
    assert!(is_illegal(&send_message_request("u", "t", "").unwrap_err()));
    assert!(send_message_request("u", &"t".repeat(50), "b").is_ok());
    assert!(is_illegal(
        &send_message_request("u", &"t".repeat(51), "b").unwrap_err()
    ));
    // characters outside the BMP count once, not as two UTF-16 units
    assert!(send_message_request("u", &"\u{1F600}".repeat(50), "b").is_ok());
    assert!(is_illegal(
        &send_message_request("u", &"\u{1F600}".repeat(51), "b").unwrap_err()
    ));
    assert!(send_message_request("u", "t", &"\u{1F600}".repeat(900)).is_ok());
    assert!(send_message_request("u", "t", &"b".repeat(900)).is_ok());
    assert!(is_illegal(
        &send_message_request("u", "t", &"b".repeat(901)).unwrap_err()
    ));
}

#[test]
fn encoding_names() {
    assert_eq!(Encoding::default().name(), "UTF8");
    assert_eq!(Encoding::Utf16.name(), "UTF-16");
    assert_eq!(Encoding::Koi8R.name(), "KOI8_R");
}
