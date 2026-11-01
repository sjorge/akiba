//! The FILE command of the specification, looked up by ed2k hash: the request
//! and the decoded reply. See the crate root for the link.
//!
//! The reply is `220 FILE` followed by one data line, `fid|` and then the
//! fields of the fmask in mask order, then the fields of the amask. See
//! [`FileReply::decode`].

use crate::error::Error;
use crate::fields;
use crate::mask::{decode_fields, AnimeFields, FileFields};
use crate::wire::Request;

pub use crate::types::file::*;

/// The file ids of a 322 MULTIPLE FILES FOUND reply, sent as `|` separated
/// numbers. The lines are read in order and empty items are dropped.
///
/// # Errors
///
/// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
pub fn multiple_files(lines: &[Vec<String>]) -> Result<Vec<i64>, Error> {
    lines
        .iter()
        .flatten()
        .filter(|item| !item.is_empty())
        .map(|item| fields::int("fid", item))
        .collect()
}

/// Builds `FILE ed2k=&size=&fmask=&amask=` for one file.
#[must_use]
pub fn request(ed2k: &str, size: u64, file: &FileFields, anime: &AnimeFields) -> Request {
    Request::new("FILE")
        .param("ed2k", ed2k)
        .param("size", size.to_string())
        .param("fmask", file.to_hex())
        .param("amask", anime.to_hex())
}

impl FileReply {
    /// Decodes the first data line of a `220 FILE` reply. `file` and `anime`
    /// must be the sets the request was built with, since the reply does not
    /// name its fields.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
    pub fn decode(line: &[String], file: &FileFields, anime: &AnimeFields) -> Result<Self, Error> {
        let (fid, rest) = line
            .split_first()
            .ok_or_else(|| Error::BadData("FILE reply has no data".to_owned()))?;
        let fid = fid
            .parse::<i64>()
            .map_err(|_| Error::BadData(format!("FILE reply fid: {fid:?}")))?;

        let values: Vec<&str> = rest.iter().map(String::as_str).collect();
        let (file_count, anime_count) = (file.len(), anime.len());
        if values.len() < file_count + anime_count {
            return Err(Error::BadData(format!(
                "reply has {} fields, {} expected",
                values.len(),
                file_count + anime_count
            )));
        }
        Ok(Self {
            fid,
            file: decode_fields(*file, &values[..file_count])?,
            anime: decode_fields(*anime, &values[file_count..file_count + anime_count])?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mask::{AnimeField, FileField};

    #[test]
    fn request_sends_the_masks_in_hex() {
        let file: FileFields = [FileField::Size, FileField::Ed2k].into_iter().collect();
        let anime: AnimeFields = [AnimeField::Year].into_iter().collect();
        let request = request("abc", 42, &file, &anime);
        assert_eq!(
            request.encode(None, "T"),
            "FILE ed2k=abc&size=42&fmask=00c0000000&amask=20000000&tag=T"
        );
    }

    #[test]
    fn multiple_files_are_numbers_across_lines() {
        let lines = vec![
            ["1628637", "1628638"].map(str::to_owned).to_vec(),
            vec![String::new()],
            ["2"].map(str::to_owned).to_vec(),
        ];
        assert_eq!(
            multiple_files(&lines).unwrap(),
            vec![1_628_637, 1_628_638, 2]
        );
        assert!(matches!(
            multiple_files(&[vec!["12x".to_owned()]]),
            Err(Error::BadData(_))
        ));
    }
}
