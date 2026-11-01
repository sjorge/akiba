//! The ANIME command of the specification: its seven byte amask, and the reply,
//! which holds the requested fields in mask order. See the crate root for the
//! link.
//!
//! This is not the FILE amask ([`crate::mask::AnimeField`]), whose bytes and bits
//! mean something else. `aid` is always
//! requested, so [`mask`] adds it and every reply starts with it.
//!
//! Retired, unused and reserved bits are not in the table, because the server
//! answers 505 when they are set. Building the request itself belongs to the
//! client.

use crate::error::Error;
use crate::mask::{decode_fields, Value};

pub use crate::types::anime::*;

/// The amask for the given fields, with `aid` added, as `AnimeParser` does.
#[must_use]
pub fn mask(fields: impl IntoIterator<Item = AnimeCommandField>) -> AnimeCommandFields {
    let mut set: AnimeCommandFields = fields.into_iter().collect();
    set.insert(AnimeCommandField::Aid);
    set
}

/// The fields `AnimeParser` requests when none are named.
#[must_use]
pub fn default_mask() -> AnimeCommandFields {
    mask([
        AnimeCommandField::Year,
        AnimeCommandField::Type,
        AnimeCommandField::RomanjiName,
        AnimeCommandField::KanjiName,
        AnimeCommandField::EnglishName,
        AnimeCommandField::OtherNameList,
        AnimeCommandField::Episodes,
        AnimeCommandField::HighestEpNumber,
        AnimeCommandField::SpecialEpCount,
        AnimeCommandField::Rating,
        AnimeCommandField::VoteCount,
        AnimeCommandField::TempRating,
        AnimeCommandField::TempVoteCount,
        AnimeCommandField::AverageReviewRating,
        AnimeCommandField::ReviewCount,
    ])
}

impl AnimeReply {
    /// Decodes the data line of a `230 ANIME` reply. `fields` must be the mask
    /// the request was built with, since the reply does not name its fields.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn decode(line: &[String], fields: &AnimeCommandFields) -> Result<Self, Error> {
        if !fields.contains(AnimeCommandField::Aid) {
            return Err(Error::BadData("ANIME mask has no aid".to_owned()));
        }
        let values: Vec<&str> = line.iter().map(String::as_str).collect();
        let decoded = decode_fields(*fields, &values)?;
        let aid = decoded
            .get(&AnimeCommandField::Aid)
            .and_then(Value::as_int)
            .ok_or_else(|| Error::BadData("ANIME reply has no aid".to_owned()))?;
        Ok(Self {
            aid,
            fields: decoded,
        })
    }
}

#[cfg(test)]
mod tests;
