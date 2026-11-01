//! The MYLIST command of the specification. Its reply is the entry of one file
//! in the mylist (221), which is also the
//! record that comes with 310 FILE ALREADY IN MYLIST.
//!
//! The data line is `lid|fid|eid|aid|gid|date|state|viewdate|storage|source|other|filestate`.
//! `other` is the only field with newlines, which `AniDB` sends as `<br />`.
//!
//! A MYLIST by anime can also answer 312 MULTIPLE MYLIST ENTRIES with a summary
//! of several entries, see [`MylistByAnime`]. MYLISTSTATS is in
//! [`crate::responses`].
//!
//! The replies of the MYLISTADD and MYLISTDEL writes (210, 211, 310, 311) are
//! decoded next to their requests, in [`crate::commands`].

use crate::codes::ResponseCode;
use crate::error::Error;
use crate::fields::{self, coded};

pub use crate::types::mylist::*;

const FIELD_COUNT: usize = 12;

impl MylistEntry {
    /// Decodes the fields of the data line of a MYLIST reply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(fields_of_line: &[String]) -> Result<Self, Error> {
        if fields_of_line.len() < FIELD_COUNT {
            return Err(Error::BadData(format!(
                "reply has {} fields, {FIELD_COUNT} expected",
                fields_of_line.len()
            )));
        }
        let raw = |index: usize| fields_of_line[index].as_str();
        Ok(Self {
            lid: fields::int("lid", raw(0))?,
            fid: fields::int("fid", raw(1))?,
            eid: fields::int("eid", raw(2))?,
            aid: fields::int("aid", raw(3))?,
            gid: fields::int("gid", raw(4))?,
            date: fields::int("date", raw(5))?,
            state: coded("state", raw(6), MylistState::from_code)?,
            viewdate: fields::int("viewdate", raw(7))?,
            storage: fields::line_breaks(raw(8)),
            source: fields::line_breaks(raw(9)),
            other: fields::line_breaks(raw(10)),
            filestate: coded("filestate", raw(11), MylistFileState::from_code)?,
        })
    }
}

const SUMMARY_HEAD: usize = 7;

impl MylistSummary {
    /// Decodes the data line of a 312 reply. A group without its episode list is
    /// an error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(fields_of_line: &[String]) -> Result<Self, Error> {
        if fields_of_line.len() < SUMMARY_HEAD {
            return Err(Error::BadData(format!(
                "reply has {} fields, {SUMMARY_HEAD} expected",
                fields_of_line.len()
            )));
        }
        let head = &fields_of_line[..SUMMARY_HEAD];
        let groups = &fields_of_line[SUMMARY_HEAD..];
        if !groups.len().is_multiple_of(2) {
            return Err(Error::BadData(format!(
                "group {:?} has no episode list",
                groups[groups.len() - 1]
            )));
        }
        Ok(Self {
            anime_title: fields::line_breaks(&head[0]),
            episodes: fields::int("episodes", &head[1])?,
            eps_unknown: head[2].clone(),
            eps_hdd: head[3].clone(),
            eps_cd: head[4].clone(),
            eps_deleted: head[5].clone(),
            eps_watched: head[6].clone(),
            groups: groups
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[group, episodes]| MylistGroup {
                    group: fields::line_breaks(group),
                    episodes: episodes.clone(),
                })
                .collect(),
        })
    }
}

impl MylistByAnime {
    /// Decodes the data line of a 221 or 312 reply, chosen by `code`. Any other
    /// code is an error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn parse(code: ResponseCode, fields_of_line: &[String]) -> Result<Self, Error> {
        match u16::from(code) {
            221 => MylistEntry::parse(fields_of_line).map(Self::Single),
            312 => MylistSummary::parse(fields_of_line).map(Self::Multiple),
            _ => Err(Error::BadData(format!("{code} is not a mylist reply"))),
        }
    }
}

#[cfg(test)]
mod tests;
