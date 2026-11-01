//! Value parsers for the fields of the replies.
//!
//! An empty integer field is `0`, because `AniDB` sends an empty field for an
//! integer that is not set. Anything else that does not parse is an
//! [`Error::BadData`].

use crate::error::Error;

pub(crate) use crate::types::dateflag::*;

fn bad(name: &str, value: &str) -> Error {
    Error::BadData(format!("{name}: not a valid value: {value:?}"))
}

/// An integer field. An empty value is `0`.
pub(crate) fn int(name: &str, value: &str) -> Result<i64, Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    trimmed.parse().map_err(|_| bad(name, value))
}

/// A boolean field sent as `1` or `0`.
pub(crate) fn boolean(name: &str, value: &str) -> Result<bool, Error> {
    Ok(int(name, value)? != 0)
}

/// A list of numbers separated by `sep`. `AniDB` separates the items of most
/// lists with a single quote, a few fields (categories, tags, character ids)
/// use a comma. Empty items are dropped, which is how the FILE and MYLIST
/// replies read their lists.
pub(crate) fn int_list(name: &str, value: &str, sep: char) -> Result<Vec<i64>, Error> {
    value
        .split(sep)
        .filter(|v| !v.is_empty())
        .map(|v| int(name, v))
        .collect()
}

/// A list of text separated by `sep`. Empty items are dropped.
pub(crate) fn str_list(value: &str, sep: char) -> Vec<String> {
    value
        .split(sep)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A list of text separated by `sep`, like [`int_list`], with the line breaks of
/// every item turned into `\n` (see [`line_breaks`]).
pub(crate) fn text_list(value: &str, sep: char) -> Vec<String> {
    value
        .split(sep)
        .filter(|v| !v.is_empty())
        .map(line_breaks)
        .collect()
}

/// Splits a record on `sep` without dropping anything, so every part of a
/// nested record is kept.
pub(crate) fn split(value: &str, sep: char) -> Vec<String> {
    value.split(sep).map(str::to_owned).collect()
}

/// A coded field: an empty value gives `None`, and so does a code that the
/// table does not know. Junk that is not a number is an error.
pub(crate) fn coded<T>(
    name: &str,
    raw: &str,
    from: fn(i64) -> Option<T>,
) -> Result<Option<T>, Error> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    Ok(from(int(name, raw)?))
}

/// The fields of one record, which must hold at least `count` of them. Extra
/// trailing fields are ignored, the spec says new ones get added at the end.
pub(crate) fn record<S: AsRef<str>>(fields: &[S], count: usize) -> Result<&[S], Error> {
    if fields.len() < count {
        return Err(Error::BadData(format!(
            "reply has {} fields, {count} expected",
            fields.len()
        )));
    }
    Ok(&fields[..count])
}

/// The records of a reply with one record per line. Empty lines are skipped.
pub(crate) fn record_lines(lines: &[Vec<String>]) -> impl Iterator<Item = &[String]> {
    lines
        .iter()
        .map(Vec::as_slice)
        .filter(|line| !(line.len() == 1 && line[0].is_empty()))
}

/// The data of a reply as the server sent it: the fields of a line joined with
/// `|`, the lines joined with `\n`. Used where the text itself is the value.
pub(crate) fn text(lines: &[Vec<String>]) -> String {
    lines
        .iter()
        .map(|line| line.join("|"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The flags set in a date flags value, in bit order. An empty value gives no
/// flags, bits above 6 are ignored, and junk is an error.
pub(crate) fn dateflags(name: &str, value: &str) -> Result<Vec<DateFlag>, Error> {
    let bits = int(name, value)?;
    Ok(DateFlag::ALL
        .into_iter()
        .enumerate()
        .filter(|(bit, _)| (bits >> bit) & 1 == 1)
        .map(|(_, flag)| flag)
        .collect())
}

/// Turns `AniDB`'s `<br />` line breaks into `\n`, matching the tag without
/// regard to case. The spec says content newlines are encoded this way in every
/// returned field, so every free-text field of a reply goes through here. The spec also says replies carry a backtick for `'` and a slash
/// for `|`, but replies contain real apostrophes, so those are not undone.
#[must_use]
pub(crate) fn line_breaks(value: &str) -> String {
    const TAG: &[u8] = b"<br />";
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    // `last` and `i` only ever sit on ASCII boundaries: the start of the text,
    // the `<` of a tag, or the byte after a tag.
    let mut last = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes.len() - i >= TAG.len() && bytes[i..i + TAG.len()].eq_ignore_ascii_case(TAG) {
            out.push_str(&value[last..i]);
            out.push('\n');
            i += TAG.len();
            last = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&value[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_integers_are_zero_and_junk_is_an_error() {
        assert_eq!(int("x", "").unwrap(), 0);
        assert_eq!(int("x", " 12 ").unwrap(), 12);
        assert_eq!(int("x", "-1").unwrap(), -1);
        assert!(matches!(int("x", "12a"), Err(Error::BadData(_))));
    }

    #[test]
    fn booleans_follow_zero_and_nonzero() {
        assert!(!boolean("b", "0").unwrap());
        assert!(boolean("b", "1").unwrap());
        assert!(!boolean("b", "").unwrap());
    }

    #[test]
    fn lists_drop_empty_items() {
        assert_eq!(text_list("", '\''), Vec::<String>::new());
        assert_eq!(text_list("a''b'", '\''), vec!["a", "b"]);
        assert_eq!(text_list("a<br />b,c", ','), vec!["a\nb", "c"]);
        assert_eq!(int_list("l", "1,,2", ',').unwrap(), vec![1, 2]);
        assert!(matches!(int_list("l", "1,x", ','), Err(Error::BadData(_))));
        assert_eq!(str_list("a'b'", '\''), vec!["a", "b"]);
        assert_eq!(split("a,,b", ','), vec!["a", "", "b"]);
    }

    #[test]
    fn coded_fields_follow_the_table() {
        fn state(code: i64) -> Option<bool> {
            (code == 1).then_some(true)
        }
        assert_eq!(coded("c", "", state).unwrap(), None);
        assert_eq!(coded("c", " ", state).unwrap(), None);
        assert_eq!(coded("c", "1", state).unwrap(), Some(true));
        assert_eq!(coded("c", "9", state).unwrap(), None);
        assert!(matches!(coded("c", "x", state), Err(Error::BadData(_))));
    }

    #[test]
    fn records_need_their_fields_and_keep_the_first_ones() {
        let fields: Vec<String> = split("a|b|c", '|');
        assert_eq!(record(&fields, 2).unwrap(), &fields[..2]);
        assert!(matches!(record(&fields, 4), Err(Error::BadData(_))));
    }

    #[test]
    fn record_lines_skip_empty_lines() {
        let lines = vec![split("1|2", '|'), vec![String::new()], split("3|4", '|')];
        let kept: Vec<&[String]> = record_lines(&lines).collect();
        assert_eq!(kept.len(), 2);
        assert_eq!(text(&lines), "1|2\n\n3|4");
    }

    #[test]
    fn dateflags_follow_the_bits() {
        assert_eq!(dateflags("d", "").unwrap(), vec![]);
        assert_eq!(dateflags("d", "0").unwrap(), vec![]);
        assert_eq!(
            dateflags("d", "5").unwrap(),
            vec![DateFlag::StartUnknownDay, DateFlag::EndUnknownDay]
        );
        assert_eq!(dateflags("d", "128").unwrap(), vec![]);
        assert_eq!(dateflags("d", "-1").unwrap().len(), 7);
        assert!(matches!(dateflags("d", "abc"), Err(Error::BadData(_))));
    }

    #[test]
    fn line_breaks_ignore_case() {
        assert_eq!(line_breaks("a<br />b<BR />c<Br />d"), "a\nb\nc\nd");
        assert_eq!(line_breaks("no breaks"), "no breaks");
        assert_eq!(line_breaks("<br />"), "\n");
        assert_eq!(line_breaks("<br/>"), "<br/>");
        assert_eq!(line_breaks("é<br />ü"), "é\nü");
    }
}
