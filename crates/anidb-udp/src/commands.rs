//! The remaining commands of the client, one typed method each.
//!
//! Every method builds a [`Request`], sends it through [`Client::call`] and
//! decodes the reply with the parsers of [`crate::responses`], [`crate::anime`],
//! [`crate::mylist`], [`crate::file`] and [`crate::simple`]. The parameters keep
//! a fixed order, so the request lines are always the same.
//!
//! Parameter validation fails with [`Error::InvalidArgument`] (a usage error)
//! before anything is sent. Many of the
//! rules cannot be broken here, since the types leave no room: a vote is a
//! [`VoteChoice`], UPDATED takes an [`UpdatedSince`] (so never both `age` and
//! `time`), and a MYLISTADD target is a [`MylistFile`].

use crate::anime::{self, AnimeCommandField, AnimeCommandFields, AnimeReply};
use crate::client::{first_line, Client};
use crate::codes::ResponseCode;
use crate::error::Error;
use crate::file::{self, FileReply};
use crate::mask::{AnimeFields, FileFields};
use crate::mylist::{MylistByAnime, MylistEntry, MylistState};
use crate::responses::{
    join_description, AnimeDesc, CalendarEntry, Character, CompletionState, Creator, Episode,
    Group, GroupStatusEntry, MylistStats, RandomAnime, Updated, UserId, VoteRecord, VoteResult,
    VoteType,
};
use crate::simple;
use crate::wire::{Request, Response};

pub use crate::types::commands::*;

/// Longest title of a message, counted in characters (Unicode scalar values).
/// The same goes for the body.
const MESSAGE_TITLE_MAX: usize = 50;
/// Longest body of a message.
const MESSAGE_BODY_MAX: usize = 900;

fn invalid_argument(message: &str) -> Error {
    Error::InvalidArgument(message.to_owned())
}

/// Adds `id_key=` or `name_key=` to a request.
fn target(request: Request, value: IdOrName<'_>, id_key: &str, name_key: &str) -> Request {
    match value {
        IdOrName::Id(id) => request.param(id_key, id.to_string()),
        IdOrName::Name(name) => request.param(name_key, name),
    }
}

const fn state_code(state: CompletionState) -> u8 {
    match state {
        CompletionState::Ongoing => 1,
        CompletionState::Stalled => 2,
        CompletionState::Complete => 3,
        CompletionState::Dropped => 4,
        CompletionState::Finished => 5,
        CompletionState::SpecialsOnly => 6,
    }
}

const fn mylist_state_code(state: MylistState) -> u8 {
    match state {
        MylistState::Unknown => 0,
        MylistState::InternalStorage => 1,
        MylistState::ExternalStorage => 2,
        MylistState::Deleted => 3,
        MylistState::RemoteStorage => 4,
    }
}

const fn vote_type_code(vote_type: VoteType) -> u8 {
    match vote_type {
        VoteType::Anime => 1,
        VoteType::AnimeTmp => 2,
        VoteType::Group => 3,
        VoteType::Episode => 6,
    }
}

/// The number a 210, 211 or 311 reply carries on its data line.
fn count(response: &Response, name: &str) -> Result<i64, Error> {
    let field = first_line(response)?
        .first()
        .ok_or_else(|| Error::BadData(format!("{} reply has no data", response.code.code())))?;
    crate::fields::int(name, field)
}

/// The count of a 311 reply. The spec lists two forms of 311: the bare reply,
/// and one followed by the number of entries edited. A bare reply counts as one.
fn edited_count(response: &Response, name: &str) -> Result<i64, Error> {
    if response.lines.iter().flatten().all(String::is_empty) {
        Ok(1)
    } else {
        count(response, name)
    }
}

/// Options of a MYLISTADD, in a fixed order.
fn add_options(mut request: Request, options: &MylistAddOptions) -> Request {
    let flag = |value: bool| if value { "1" } else { "0" };
    // by lid `edit=1` is in the request already
    let has_edit = request.params.iter().any(|(key, _)| key == "edit");
    if let (Some(edit), false) = (options.edit, has_edit) {
        request = request.param("edit", flag(edit));
    }
    if let Some(state) = options.state {
        request = request.param("state", mylist_state_code(state).to_string());
    }
    if let Some(viewed) = options.viewed {
        request = request.param("viewed", flag(viewed));
    }
    if let Some(viewdate) = options.viewdate {
        request = request.param("viewdate", viewdate.to_string());
    }
    if let Some(source) = &options.source {
        request = request.param("source", source.as_str());
    }
    if let Some(storage) = &options.storage {
        request = request.param("storage", storage.as_str());
    }
    if let Some(other) = &options.other {
        request = request.param("other", other.as_str());
    }
    request
}

fn file_spec(request: Request, file: MylistFile<'_>) -> Request {
    match file {
        MylistFile::Fid(fid) => request.param("fid", fid.to_string()),
        MylistFile::Hash { ed2k, size } => {
            request.param("ed2k", ed2k).param("size", size.to_string())
        }
        MylistFile::Lid(lid) => request.param("lid", lid.to_string()),
    }
}

fn mylist_add_file_request(
    file: MylistFile<'_>,
    options: &MylistAddOptions,
) -> Result<Request, Error> {
    let mut request = file_spec(Request::new("MYLISTADD"), file);
    if matches!(file, MylistFile::Lid(_)) {
        if options.edit == Some(false) {
            return Err(invalid_argument(
                "when passing my list id, options.edit must be set to true",
            ));
        }
        request = request.param("edit", "1");
    }
    Ok(add_options(request, options))
}

fn mylist_add_anime_request(
    spec: &MylistAnimeSpec<'_>,
    options: &MylistAddOptions,
) -> Result<Request, Error> {
    if spec.anime.is_empty() {
        return Err(invalid_argument(
            "Invalid first argument: animeinfo.anime property and animeinfo.epno properties are required",
        ));
    }
    let mut request = target(Request::new("MYLISTADD"), spec.anime, "aid", "aname");
    request = match spec.group {
        Some(group) => target(request, group, "gid", "gname"),
        // Deliberate deviation from the spec, which makes the group optional only
        // for `edit=1` and offers `generic=1` as an alternative groupinfo: adding
        // an episode without a group sends `generic=1`, so that a file whose hash
        // AniDB does not know yet shows up in the mylist as a generic entry
        // instead of not at all.
        None => request.param("generic", "1"),
    };
    request = request.param("epno", spec.epno.to_string());
    Ok(add_options(request, options))
}

fn mylist_del_anime_request(
    anime: IdOrName<'_>,
    group: Option<IdOrName<'_>>,
    epno: Option<i64>,
) -> Result<Request, Error> {
    if anime.is_empty() {
        return Err(invalid_argument(
            "Invalid first argument: animeinfo.anime is required and animeinfo.epno must be an integer when given",
        ));
    }
    let mut request = target(Request::new("MYLISTDEL"), anime, "aid", "aname");
    if let Some(group) = group {
        request = target(request, group, "gid", "gname");
    }
    // without an epno every matching entry is deleted
    if let Some(epno) = epno {
        request = request.param("epno", epno.to_string());
    }
    Ok(request)
}

fn vote_request(
    vote_type: VoteType,
    target_of_vote: IdOrName<'_>,
    choice: VoteChoice,
    epno: Option<&str>,
) -> Result<Request, Error> {
    if epno.is_some() && vote_type != VoteType::Anime {
        return Err(invalid_argument(
            "epno can only be used when voting for an anime",
        ));
    }
    let mut request = Request::new("VOTE").param("type", vote_type_code(vote_type).to_string());
    request = target(request, target_of_vote, "id", "name");
    if let Some(epno) = epno {
        request = request.param("epno", epno);
    }
    let value = match choice {
        VoteChoice::Get => 0,
        VoteChoice::Revoke => -1,
        VoteChoice::Value(value) if (100..=1000).contains(&value) => i32::from(value),
        VoteChoice::Value(value) => {
            return Err(invalid_argument(&format!(
                "vote ({value}) must must be 'get' or 'revoke' or a number between 100 and 1000 "
            )));
        }
    };
    Ok(request.param("value", value.to_string()))
}

fn send_message_request(username: &str, title: &str, body: &str) -> Result<Request, Error> {
    if username.is_empty() || title.is_empty() || body.is_empty() {
        return Err(invalid_argument(
            "Fields username, title, and body are required and must be a string",
        ));
    }
    let title_length = title.chars().count();
    if title_length > MESSAGE_TITLE_MAX {
        return Err(invalid_argument(&format!(
            "title must not be longer than 50 characters but was {title_length}"
        )));
    }
    let body_length = body.chars().count();
    if body_length > MESSAGE_BODY_MAX {
        return Err(invalid_argument(&format!(
            "body must not be longer than 900 characters but was {body_length}"
        )));
    }
    Ok(Request::new("SENDMSG")
        .param("to", username)
        .param("title", title)
        .param("body", body))
}

fn anime_request(anime: IdOrName<'_>, fields: AnimeCommandFields) -> Request {
    let request = Request::new("ANIME").param("amask", fields.to_hex());
    target(request, anime, "aid", "aname")
}

fn file_request(request: Request, file: FileFields, anime: AnimeFields) -> Request {
    request
        .param("fmask", file.to_hex())
        .param("amask", anime.to_hex())
}

impl Client {
    /// `ENCODING name=`: switches the server encoding, and replies 219
    /// ENCODING CHANGED. Needs no session. The encoding is reset to the default
    /// on logout. The spec prefers `enc=` of AUTH and calls ENCODING mostly for
    /// testing; [`Client::connect`] always sets `enc=UTF8`. 519 (not supported) is an error.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn encoding(&mut self, encoding: Encoding) -> Result<(), Error> {
        self.call(
            &Request::new("ENCODING").param("name", encoding.name()),
            &[ResponseCode::EncodingChanged],
        )?;
        Ok(())
    }

    /// VERSION: the server version. Needs no session.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn version(&mut self) -> Result<String, Error> {
        let response = self.call(&Request::new("VERSION"), &[ResponseCode::Version])?;
        Ok(simple::version(&response.lines))
    }

    /// UPTIME: the server uptime in milliseconds. The spec calls it the
    /// preferred way to check that the session is still open and to keep it
    /// alive, since the server drops a session after 35 minutes of silence.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn uptime(&mut self) -> Result<i64, Error> {
        let response = self.call(&Request::new("UPTIME"), &[ResponseCode::Uptime])?;
        simple::uptime(first_line(&response)?)
    }

    /// ANIME by id or exact title, with an amask. `None` asks for the default
    /// fields of [`anime::default_mask`]. `aid` is always requested. 330 when the
    /// anime is unknown.
    ///
    /// - By name the title must match exactly: romaji, kanji, english, other,
    ///   synonym or short name.
    /// - The fields come back in mask order (byte 1 bit 7 first). A retired,
    ///   unused or reserved bit gives 505.
    ///
    /// NOTE: when the reply would not fit one packet, the character id list is
    /// cut first, then the tag list, the synonym list and the short name list.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn anime(
        &mut self,
        anime: IdOrName<'_>,
        fields: Option<&AnimeCommandFields>,
    ) -> Result<AnimeReply, Error> {
        let mut fields = fields.copied().unwrap_or_else(anime::default_mask);
        fields.insert(AnimeCommandField::Aid);
        let response = self.call(&anime_request(anime, fields), &[ResponseCode::Anime])?;
        AnimeReply::decode(first_line(&response)?, &fields)
    }

    /// ANIMEDESC: the whole description of an anime, fetched part by part and
    /// joined, with `<br />` turned into newlines. 330 and 333 when there is no
    /// such anime or description.
    ///
    /// A description can be about 5000 characters, but a packet holds about
    /// 1400 bytes, so the text comes in zero based parts. There is no lookup by
    /// title, only by `aid`.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn anime_desc(&mut self, aid: i64) -> Result<String, Error> {
        let part = |client: &mut Self, part: i64| -> Result<AnimeDesc, Error> {
            let request = Request::new("ANIMEDESC")
                .param("aid", aid.to_string())
                .param("part", part.to_string());
            let response = client.call(&request, &[ResponseCode::AnimeDescription])?;
            AnimeDesc::parse(&response.lines)
        };
        let first = part(self, 0)?;
        let mut parts = vec![first];
        for number in 1..parts[0].max_parts {
            parts.push(part(self, number)?);
        }
        Ok(join_description(&parts))
    }

    /// CALENDAR: the 25 most recently aired and the next 25 titles, ordered by
    /// start date. Takes no parameters. There may be fewer than 50 titles, since
    /// adult titles are filtered by the preference of the logged in user. 397
    /// (nothing to show) is an empty list.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn calendar(&mut self) -> Result<Vec<CalendarEntry>, Error> {
        let response = self.call(
            &Request::new("CALENDAR"),
            &[ResponseCode::Calendar, ResponseCode::CalendarEmpty],
        )?;
        if response.code == ResponseCode::CalendarEmpty {
            return Ok(Vec::new());
        }
        CalendarEntry::parse_lines(&response.lines)
    }

    /// CHARACTER by id. 335 when unknown.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn character(&mut self, charid: i64) -> Result<Character, Error> {
        let request = Request::new("CHARACTER").param("charid", charid.to_string());
        let response = self.call(&request, &[ResponseCode::Character])?;
        Character::parse(first_line(&response)?)
    }

    /// CREATOR by id. 345 when unknown.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn creator(&mut self, creatorid: i64) -> Result<Creator, Error> {
        let request = Request::new("CREATOR").param("creatorid", creatorid.to_string());
        let response = self.call(&request, &[ResponseCode::Creator])?;
        Creator::parse(first_line(&response)?)
    }

    /// EPISODE by episode id. 340 when unknown.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn episode(&mut self, eid: i64) -> Result<Episode, Error> {
        let request = Request::new("EPISODE").param("eid", eid.to_string());
        let response = self.call(&request, &[ResponseCode::Episode])?;
        Episode::parse(first_line(&response)?)
    }

    /// EPISODE by anime (id or title) and episode number, such as `12` or `S1`.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn episode_by_anime(&mut self, anime: IdOrName<'_>, epno: &str) -> Result<Episode, Error> {
        let request = target(
            Request::new("EPISODE").param("epno", epno),
            anime,
            "aid",
            "aname",
        );
        let response = self.call(&request, &[ResponseCode::Episode])?;
        Episode::parse(first_line(&response)?)
    }

    /// GROUP by id, or by exact name or short name. Needs a login. 350 when
    /// unknown.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn group(&mut self, group: IdOrName<'_>) -> Result<Group, Error> {
        let request = target(Request::new("GROUP"), group, "gid", "gname");
        let response = self.call(&request, &[ResponseCode::Group])?;
        Group::parse(first_line(&response)?)
    }

    /// GROUPSTATUS: the groups that release an anime. Without a `state` the
    /// ongoing, finished and complete groups. 325 when none are found, 330 when
    /// the anime is unknown.
    ///
    /// The groups are filtered by the languages of the user's profile, and
    /// sorted by descending episode count. Groups that do not fit one packet are
    /// silently dropped by the server.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn groupstatus(
        &mut self,
        aid: i64,
        state: Option<CompletionState>,
    ) -> Result<Vec<GroupStatusEntry>, Error> {
        let mut request = Request::new("GROUPSTATUS").param("aid", aid.to_string());
        if let Some(state) = state {
            request = request.param("state", state_code(state).to_string());
        }
        let response = self.call(&request, &[ResponseCode::GroupStatus])?;
        GroupStatusEntry::parse_lines(&response.lines)
    }

    /// FILE by file id, decoded with the given masks. 320 when unknown.
    ///
    /// `fid` always comes first in the reply, whatever the masks are. The other
    /// fields follow the order of the mask tables, fmask before amask. There is
    /// no way to ask for all fields. A retired, unused or reserved bit gives
    /// 505. Audio codec and bitrate hold several values when the file has
    /// several audio streams, separated by a single quote.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn file(
        &mut self,
        fid: i64,
        file: &FileFields,
        anime: &AnimeFields,
    ) -> Result<FileReply, Error> {
        let request = file_request(
            Request::new("FILE").param("fid", fid.to_string()),
            *file,
            *anime,
        );
        let response = self.call(&request, &[ResponseCode::File])?;
        FileReply::decode(first_line(&response)?, file, anime)
    }

    /// FILE by anime, group and episode number. The server returns the first
    /// matching file, and 322 with all of them is [`Error::MultipleFilesFound`].
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn file_by_episode(
        &mut self,
        anime: IdOrName<'_>,
        group: IdOrName<'_>,
        epno: &str,
        file: &FileFields,
        anime_fields: &AnimeFields,
    ) -> Result<FileReply, Error> {
        let mut request = file_request(
            Request::new("FILE").param("epno", epno),
            *file,
            *anime_fields,
        );
        request = target(request, anime, "aid", "aname");
        request = target(request, group, "gid", "gname");
        let response = self.call(
            &request,
            &[ResponseCode::File, ResponseCode::MultipleFilesFound],
        )?;
        if response.code == ResponseCode::MultipleFilesFound {
            return Err(Error::MultipleFilesFound {
                fids: file::multiple_files(&response.lines)?,
            });
        }
        FileReply::decode(first_line(&response)?, file, anime_fields)
    }

    /// UPDATED: the anime updated since an age in days or a unix time. 343 (no
    /// updates) is [`Updated::no_updates`].
    ///
    /// An anime counts as updated when its record changes, when a main or
    /// official title is added, edited or deleted, when an episode is added or
    /// deleted (an edit does not count), when an episode title changes, or when a
    /// relation is added or deleted.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn updated(&mut self, since: UpdatedSince) -> Result<Updated, Error> {
        let request = Request::new("UPDATED").param("entity", "1");
        let request = match since {
            UpdatedSince::Age(days) => request.param("age", days.to_string()),
            UpdatedSince::Time(time) => request.param("time", time.to_string()),
        };
        let response = self.call(
            &request,
            &[ResponseCode::Updated, ResponseCode::NoSuchUpdates],
        )?;
        if response.code == ResponseCode::NoSuchUpdates {
            return Ok(Updated::no_updates());
        }
        Updated::parse(first_line(&response)?)
    }

    /// MYLIST by mylist entry id. 321 when unknown.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_by_id(&mut self, lid: i64) -> Result<MylistEntry, Error> {
        let request = Request::new("MYLIST").param("lid", lid.to_string());
        let response = self.call(&request, &[ResponseCode::Mylist])?;
        MylistEntry::parse(first_line(&response)?)
    }

    /// MYLIST by file id. 321 when the file is not in the mylist.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_by_file_id(&mut self, fid: i64) -> Result<MylistEntry, Error> {
        let request = Request::new("MYLIST").param("fid", fid.to_string());
        let response = self.call(&request, &[ResponseCode::Mylist])?;
        MylistEntry::parse(first_line(&response)?)
    }

    /// MYLIST by anime, with an optional group and episode number (0 is the
    /// same as none). One entry (221) or the summary of several (312).
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_by_anime(
        &mut self,
        anime: IdOrName<'_>,
        group: Option<IdOrName<'_>>,
        epno: Option<i64>,
    ) -> Result<MylistByAnime, Error> {
        let mut request = Request::new("MYLIST");
        if let Some(epno) = epno.filter(|epno| *epno != 0) {
            request = request.param("epno", epno.to_string());
        }
        request = target(request, anime, "aid", "aname");
        if let Some(group) = group.filter(|group| !group.is_empty()) {
            request = target(request, group, "gid", "gname");
        }
        let response = self.call(
            &request,
            &[ResponseCode::Mylist, ResponseCode::MultipleMylistEntries],
        )?;
        MylistByAnime::parse(response.code, first_line(&response)?)
    }

    /// MYLISTADD of one file by id, hash or entry id, with the full options. 310
    /// without `edit` is [`Error::FileAlreadyInMylist`] with the current
    /// entry. [`Client::mylist_add_by_hash`] is the short form for a hash.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_add_by_file(
        &mut self,
        file: MylistFile<'_>,
        options: &MylistAddOptions,
    ) -> Result<MylistAdded, Error> {
        let request = mylist_add_file_request(file, options)?;
        let response = self.call(
            &request,
            &[
                ResponseCode::MylistEntryAdded,
                ResponseCode::FileAlreadyInMylist,
                ResponseCode::MylistEntryEdited,
            ],
        )?;
        match response.code {
            ResponseCode::MylistEntryAdded => Ok(MylistAdded::Added {
                lid: count(&response, "lid")?,
            }),
            ResponseCode::MylistEntryEdited => Ok(MylistAdded::Edited {
                edited: edited_count(&response, "nr_edited")?,
            }),
            ResponseCode::FileAlreadyInMylist => Err(Error::FileAlreadyInMylist {
                entry: Box::new(MylistEntry::parse(first_line(&response)?)?),
            }),
            code => Err(Error::from_reply(
                ResponseCode::UnknownApiError,
                &format!("Unkown response code returned {}", u16::from(code)),
            )),
        }
    }

    /// MYLISTADD by anime, group and episode number. 210 gives the number of
    /// entries added, 311 the number edited (with `edit`). 330 is an unknown
    /// anime, 350 an unknown group, and 322 means several files match.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_add_by_anime(
        &mut self,
        spec: &MylistAnimeSpec<'_>,
        options: &MylistAddOptions,
    ) -> Result<MylistAddedByAnime, Error> {
        let request = mylist_add_anime_request(spec, options)?;
        let response = self.call(
            &request,
            &[
                ResponseCode::MylistEntryAdded,
                ResponseCode::MylistEntryEdited,
            ],
        )?;
        Ok(if response.code == ResponseCode::MylistEntryEdited {
            MylistAddedByAnime::Edited(edited_count(&response, "count")?)
        } else {
            MylistAddedByAnime::Added(count(&response, "count")?)
        })
    }

    /// MYLISTDEL of one file by id, hash or entry id. Returns the number of
    /// entries removed. 411 when there is none. The command deletes all entries
    /// found.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_del_by_file(&mut self, file: MylistFile<'_>) -> Result<i64, Error> {
        let request = file_spec(Request::new("MYLISTDEL"), file);
        let response = self.call(&request, &[ResponseCode::MylistEntryDeleted])?;
        count(&response, "removed")
    }

    /// MYLISTDEL by anime with an optional group and episode number. Without an
    /// `epno` every matching entry is deleted. Returns the number removed.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_del_by_anime(
        &mut self,
        anime: IdOrName<'_>,
        group: Option<IdOrName<'_>>,
        epno: Option<i64>,
    ) -> Result<i64, Error> {
        let request = mylist_del_anime_request(anime, group, epno)?;
        let response = self.call(&request, &[ResponseCode::MylistEntryDeleted])?;
        count(&response, "removed")
    }

    /// MYLISTSTATS: the statistics of the mylist.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn mylist_stats(&mut self) -> Result<MylistStats, Error> {
        let response = self.call(&Request::new("MYLISTSTATS"), &[ResponseCode::MylistStats])?;
        MylistStats::parse(first_line(&response)?)
    }

    /// VOTE for an anime, group or episode (by id or name), or retrieve or
    /// revoke the vote. `epno` is only allowed for [`VoteType::Anime`]. 360,
    /// 363 and 364 are [`Error::VoteRefused`] with the vote record.
    ///
    /// Votes are updated without asking, and a temporary vote is not possible
    /// when a permanent one exists. The reply carries `entity name|value|type|
    /// entity id`: 260 VOTED, 261 VOTE FOUND, 262 VOTE UPDATED (with the old
    /// value), 263 VOTE REVOKED. 361 and 362 are an invalid vote type and an
    /// invalid vote value. For an episode vote, pass `epno` with the anime type,
    /// or use the episode type when the episode id is known.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn vote(
        &mut self,
        vote_type: VoteType,
        subject: IdOrName<'_>,
        choice: VoteChoice,
        epno: Option<&str>,
    ) -> Result<VoteResult, Error> {
        let request = vote_request(vote_type, subject, choice, epno)?;
        let response = self.call(
            &request,
            &[
                ResponseCode::Voted,
                ResponseCode::VoteFound,
                ResponseCode::VoteUpdated,
                ResponseCode::VoteRevoked,
                ResponseCode::NoSuchVote,
                ResponseCode::PermvoteNotAllowed,
                ResponseCode::AlreadyPermvoted,
            ],
        )?;
        let code = response.code;
        if matches!(
            code,
            ResponseCode::NoSuchVote
                | ResponseCode::PermvoteNotAllowed
                | ResponseCode::AlreadyPermvoted
        ) {
            let refusal = Error::from_reply(code, &response.message);
            return Err(match response.lines.first() {
                Some(line) => Error::VoteRefused {
                    code,
                    message: refusal.to_string(),
                    vote: Box::new(VoteRecord::parse(line)?),
                },
                None => refusal,
            });
        }
        VoteResult::parse(code, first_line(&response)?)
    }

    /// RANDOMANIME: a random anime from the database or the mylist. The reply is
    /// in the format of ANIME without an amask (19 fields).
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn random(&mut self, kind: RandomType) -> Result<RandomAnime, Error> {
        let code = match kind {
            RandomType::Db => "0",
            RandomType::Watched => "1",
            RandomType::Unwatched => "2",
            RandomType::Mylist => "3",
        };
        let request = Request::new("RANDOMANIME").param("type", code);
        let response = self.call(&request, &[ResponseCode::Anime])?;
        RandomAnime::parse(first_line(&response)?)
    }

    /// MYLISTEXPORT with a template name, see [`EXPORT_TEMPLATES`]. Only one
    /// export can be queued: 318 when there is one, 317 for an unknown template.
    /// The server exports when the load is low, which can take up to 24 hours. It
    /// then sends an `AniDB` message when the export is ready.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn request_mylist_export(&mut self, template: &str) -> Result<(), Error> {
        let request = Request::new("MYLISTEXPORT").param("template", template);
        self.call(&request, &[ResponseCode::ExportQueued])?;
        Ok(())
    }

    /// MYLISTEXPORT `cancel=1`: cancels any pending export, also one queued
    /// through the website. 319 when none is queued.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn cancel_mylist_export(&mut self) -> Result<(), Error> {
        let request = Request::new("MYLISTEXPORT").param("cancel", "1");
        self.call(&request, &[ResponseCode::ExportCancelled])?;
        Ok(())
    }

    /// SENDMSG to a user, which replies 294 SENDMSG SUCCESSFUL. The title is 50
    /// characters at most (counted as Unicode characters), the body 900. 394 when the user is unknown, and 501
    /// LOGIN FIRST without a session.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn send_message(&mut self, username: &str, title: &str, body: &str) -> Result<(), Error> {
        let request = send_message_request(username, title, body)?;
        self.call(&request, &[ResponseCode::SendmessageSuccessful])?;
        Ok(())
    }

    /// USER by name or id, which replies 295 with `uid|username`. 394 when
    /// unknown.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, see [`Client::call`]: a reply code the command does not expect ([`Error::Reply`]), a timeout, a ban or an I/O failure.
    pub fn get_user_id(&mut self, user: IdOrName<'_>) -> Result<UserId, Error> {
        let request = target(Request::new("USER"), user, "uid", "user");
        let response = self.call(&request, &[ResponseCode::UserId])?;
        UserId::parse(first_line(&response)?)
    }
}

#[cfg(test)]
mod tests;
