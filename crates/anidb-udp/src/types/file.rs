//! The typed result of the FILE command. `FileReply::decode` is in
//! [`crate::file`].

use std::collections::BTreeMap;

use super::fields::{AnimeField, FileField};
use super::mask::Value;

/// A decoded FILE reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReply {
    /// The file id, always sent first in the reply.
    pub fid: i64,
    /// The fmask fields that were asked for.
    pub file: BTreeMap<FileField, Value>,
    /// The amask fields that were asked for.
    pub anime: BTreeMap<AnimeField, Value>,
}

impl FileReply {
    /// The number of an fmask field. `None` if the field was not asked for or is
    /// not a number.
    #[must_use]
    pub fn file_int(&self, field: FileField) -> Option<i64> {
        self.file.get(&field).and_then(Value::as_int)
    }

    /// The text of an fmask field, `None` if it is missing or not text.
    #[must_use]
    pub fn file_str(&self, field: FileField) -> Option<&str> {
        self.file.get(&field).and_then(Value::as_str)
    }

    /// The numbers of an fmask list field, `None` if it is missing or not a list
    /// of numbers.
    #[must_use]
    pub fn file_ints(&self, field: FileField) -> Option<&[i64]> {
        self.file.get(&field).and_then(Value::as_ints)
    }

    /// The text items of an fmask list field, `None` if it is missing or not a
    /// list of text.
    #[must_use]
    pub fn file_strs(&self, field: FileField) -> Option<&[String]> {
        self.file.get(&field).and_then(Value::as_strs)
    }

    /// The number of an amask field, like [`FileReply::file_int`].
    #[must_use]
    pub fn anime_int(&self, field: AnimeField) -> Option<i64> {
        self.anime.get(&field).and_then(Value::as_int)
    }

    /// The text of an amask field, like [`FileReply::file_str`].
    #[must_use]
    pub fn anime_str(&self, field: AnimeField) -> Option<&str> {
        self.anime.get(&field).and_then(Value::as_str)
    }

    /// The numbers of an amask list field, like [`FileReply::file_ints`].
    #[must_use]
    pub fn anime_ints(&self, field: AnimeField) -> Option<&[i64]> {
        self.anime.get(&field).and_then(Value::as_ints)
    }

    /// The text items of an amask list field, like [`FileReply::file_strs`].
    #[must_use]
    pub fn anime_strs(&self, field: AnimeField) -> Option<&[String]> {
        self.anime.get(&field).and_then(Value::as_strs)
    }
}
