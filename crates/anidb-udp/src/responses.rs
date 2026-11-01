//! The typed results of the commands whose replies are a fixed record, the
//! record types of the replies.
//!
//! Each type has a `parse` for its reply. A reply that is one record takes the
//! fields of its data line, `&[String]`. A reply with one record per line
//! (CALENDAR, GROUPSTATUS, and the text of ANIMEDESC) takes the lines,
//! `&[Vec<String>]`, as [`crate::wire::Response`] holds them.
//!
//! Field names follow the API names, except where a name is a Rust keyword
//! (`type` becomes `*_type`). An unknown code gives `None`, an unparseable number
//! is a [`Error::BadData`].
//!
//! Replies that have no record, VERSION and UPTIME, are in [`crate::simple`].
//! The MYLIST and FILE replies are in [`crate::mylist`] and [`crate::file`].
//! ANIME is in [`crate::anime`].

use crate::codes::ResponseCode;
use crate::error::Error;
use crate::fields::{
    self, boolean, coded, dateflags, int, int_list, record, record_lines, split, str_list, text,
};

pub use crate::types::responses::*;

impl AnimeDesc {
    /// Decodes an ANIMEDESC reply, `part|max parts|text`. The text is everything
    /// after the second `|`, so it may hold `|` itself.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(lines: &[Vec<String>]) -> Result<Self, Error> {
        let whole = text(lines);
        let parts: Vec<&str> = whole.splitn(3, '|').collect();
        if parts.len() < 3 {
            return Err(Error::BadData(format!(
                "reply has {} fields, 3 expected",
                parts.len()
            )));
        }
        Ok(Self {
            current_part: int("current_part", parts[0])?,
            max_parts: int("max_parts", parts[1])?,
            desc: parts[2].to_owned(),
        })
    }
}

/// Joins the parts of a description in order, then turns `<br />` into
/// newlines. The order matters: a line break can be cut in two by the part
/// size, so it is only whole after joining.
#[must_use]
pub fn join_description(parts: &[AnimeDesc]) -> String {
    let joined: String = parts.iter().map(|part| part.desc.as_str()).collect();
    fields::line_breaks(&joined)
}

impl CalendarEntry {
    /// Decodes one line of a CALENDAR reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 3)?;
        Ok(Self {
            aid: int("aid", &f[0])?,
            startdate: int("startdate", &f[1])?,
            dateflags: dateflags("dateflags", &f[2])?,
        })
    }

    /// Decodes every line of a CALENDAR reply, in order. Empty lines are skipped.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse_lines(lines: &[Vec<String>]) -> Result<Vec<Self>, Error> {
        record_lines(lines).map(Self::parse).collect()
    }
}

impl Character {
    /// Decodes a CHARACTER reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 9)?;
        Ok(Self {
            charid: int("charid", &f[0])?,
            character_name_kanji: fields::line_breaks(&f[1]),
            character_name_transc: fields::line_breaks(&f[2]),
            pic: f[3].clone(),
            anime_blocks: anime_blocks(&f[4])?,
            episode_list: int_list("episode_list", &f[5], ',')?,
            last_update_date: int("last_update_date", &f[6])?,
            character_type: int("type", &f[7])?,
            gender: f[8].clone(),
        })
    }
}

/// The anime blocks of a CHARACTER reply: `aid,appearance,creatorid,is_main_seiyuu`,
/// separated by a single quote. Without a seiyuu the last two are empty.
fn anime_blocks(raw: &str) -> Result<Vec<CharacterAnime>, Error> {
    str_list(raw, '\'')
        .iter()
        .map(|block| {
            let parts = split(block, ',');
            let f = record(&parts, 4)?;
            Ok(CharacterAnime {
                aid: int("aid", &f[0])?,
                appearance: int("appearance", &f[1])?,
                creatorid: int("creatorid", &f[2])?,
                is_main_seiyuu: boolean("is_main_seiyuu", &f[3])?,
            })
        })
        .collect()
}

impl Creator {
    /// Decodes a CREATOR reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 10)?;
        Ok(Self {
            creatorid: int("creatorid", &f[0])?,
            creator_name_kanji: fields::line_breaks(&f[1]),
            creator_name_transcription: fields::line_breaks(&f[2]),
            creator_type: coded("type", &f[3], CreatorType::from_code)?,
            pic_name: f[4].clone(),
            url_english: f[5].clone(),
            url_japanese: f[6].clone(),
            wiki_url_english: f[7].clone(),
            wiki_url_japanese: f[8].clone(),
            last_update_date: int("last_update_date", &f[9])?,
        })
    }
}

impl Episode {
    /// Decodes an EPISODE reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 11)?;
        Ok(Self {
            eid: int("eid", &f[0])?,
            aid: int("aid", &f[1])?,
            length: int("length", &f[2])?,
            rating: int("rating", &f[3])?,
            votes: int("votes", &f[4])?,
            epno: f[5].clone(),
            eng: fields::line_breaks(&f[6]),
            romaji: fields::line_breaks(&f[7]),
            kanji: fields::line_breaks(&f[8]),
            aired: int("aired", &f[9])?,
            episode_type: coded("type", &f[10], EpisodeType::from_code)?,
        })
    }
}

impl Group {
    /// Decodes a GROUP reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 17)?;
        Ok(Self {
            gid: int("gid", &f[0])?,
            rating: int("rating", &f[1])?,
            votes: int("votes", &f[2])?,
            acount: int("acount", &f[3])?,
            fcount: int("fcount", &f[4])?,
            name: fields::line_breaks(&f[5]),
            short: fields::line_breaks(&f[6]),
            irc_channel: f[7].clone(),
            irc_server: f[8].clone(),
            url: f[9].clone(),
            picname: f[10].clone(),
            foundeddate: int("foundeddate", &f[11])?,
            disbandeddate: int("disbandeddate", &f[12])?,
            dateflags: dateflags("dateflags", &f[13])?,
            lastreleasedate: int("lastreleasedate", &f[14])?,
            lastactivitydate: int("lastactivitydate", &f[15])?,
            grouprelations: group_relations(&f[16])?,
        })
    }
}

/// The relations of a GROUP reply: `othergroupid,relationtype`, separated by a
/// single quote. an empty value (a group without
/// relations) is read as no relations.
fn group_relations(raw: &str) -> Result<Vec<GroupRelation>, Error> {
    str_list(raw, '\'')
        .iter()
        .map(|item| {
            let parts = split(item, ',');
            let f = record(&parts, 2)?;
            Ok(GroupRelation {
                othergroupid: int("othergroupid", &f[0])?,
                relationtype: coded("relationtype", &f[1], RelationType::from_code)?,
            })
        })
        .collect()
}

impl GroupStatusEntry {
    /// Decodes one line of a GROUPSTATUS reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 7)?;
        Ok(Self {
            gid: int("gid", &f[0])?,
            group_name: fields::line_breaks(&f[1]),
            completion_state: coded("completion_state", &f[2], CompletionState::from_code)?,
            last_episode_number: int("last_episode_number", &f[3])?,
            rating: int("rating", &f[4])?,
            votes: int("votes", &f[5])?,
            episode_range: f[6].clone(),
        })
    }

    /// Decodes every line of a GROUPSTATUS reply, in order. Empty lines are skipped.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse_lines(lines: &[Vec<String>]) -> Result<Vec<Self>, Error> {
        record_lines(lines).map(Self::parse).collect()
    }
}

impl Updated {
    /// The result of the 343 NO UPDATES reply: nothing was updated.
    #[must_use]
    pub fn no_updates() -> Self {
        Self {
            total_count: 0,
            last_update_date: 0,
            updated_aids: Vec::new(),
        }
    }

    /// Decodes an UPDATED reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 4)?;
        int("entity", &f[0])?;
        Ok(Self {
            total_count: int("total_count", &f[1])?,
            last_update_date: int("last_update_date", &f[2])?,
            updated_aids: int_list("updated_aids", &f[3], ',')?,
        })
    }
}

impl MylistStats {
    /// Decodes a MYLISTSTATS reply, 17 integers.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 17)?;
        Ok(Self {
            anime: int("anime", &f[0])?,
            eps: int("eps", &f[1])?,
            files: int("files", &f[2])?,
            size_of_files: int("size_of_files", &f[3])?,
            added_animes: int("added_animes", &f[4])?,
            added_eps: int("added_eps", &f[5])?,
            added_files: int("added_files", &f[6])?,
            added_groups: int("added_groups", &f[7])?,
            leech_perc: int("leech_perc", &f[8])?,
            glory_perc: int("glory_perc", &f[9])?,
            viewed_perc: int("viewed_perc", &f[10])?,
            mylist_perc: int("mylist_perc", &f[11])?,
            viewed_mylist_perc: int("viewed_mylist_perc", &f[12])?,
            viewed_nr_eps: int("viewed_nr_eps", &f[13])?,
            votes: int("votes", &f[14])?,
            reviews: int("reviews", &f[15])?,
            views_in_min: int("views_in_min", &f[16])?,
        })
    }
}

impl VoteRecord {
    /// Decodes the vote record of a VOTE reply, or of one of its error replies.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 4)?;
        Ok(Self {
            name: fields::line_breaks(&f[0]),
            vote_value: int("vote_value", &f[1])?,
            vote_type: coded("type", &f[2], VoteType::from_code)?,
            id: int("id", &f[3])?,
        })
    }
}

impl VoteResult {
    /// Decodes a VOTE reply with one of the codes 260 to 263.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(code: ResponseCode, line: &[String]) -> Result<Self, Error> {
        let action = VoteAction::from_code(code)
            .ok_or_else(|| Error::BadData(format!("{code} is not a vote reply")))?;
        Ok(Self {
            action,
            vote: VoteRecord::parse(line)?,
        })
    }
}

/// A list of names separated by a single quote, with `<br />` decoded.
fn text_list(raw: &str) -> Vec<String> {
    str_list(raw, '\'')
        .iter()
        .map(|item| fields::line_breaks(item))
        .collect()
}

impl RandomAnime {
    /// Decodes a RANDOMANIME reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 19)?;
        Ok(Self {
            aid: int("aid", &f[0])?,
            episodes: int("episodes", &f[1])?,
            highest_ep_number: int("highest_ep_number", &f[2])?,
            special_ep_count: int("special_ep_count", &f[3])?,
            rating: int("rating", &f[4])?,
            vote_count: int("vote_count", &f[5])?,
            temp_rating: int("temp_rating", &f[6])?,
            temp_vote_count: int("temp_vote_count", &f[7])?,
            average_review_rating: int("average_review_rating", &f[8])?,
            review_count: int("review_count", &f[9])?,
            year: f[10].clone(),
            anime_type: f[11].clone(),
            romanji_name: fields::line_breaks(&f[12]),
            kanji_name: fields::line_breaks(&f[13]),
            english_name: fields::line_breaks(&f[14]),
            other_name_list: text_list(&f[15]),
            short_name_list: text_list(&f[16]),
            synonym_list: text_list(&f[17]),
            categories: str_list(&f[18], ','),
        })
    }
}

impl UserId {
    /// Decodes a USER reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(line: &[String]) -> Result<Self, Error> {
        let f = record(line, 2)?;
        Ok(Self {
            uid: int("uid", &f[0])?,
            username: fields::line_breaks(&f[1]),
        })
    }
}

#[cfg(test)]
mod tests;
