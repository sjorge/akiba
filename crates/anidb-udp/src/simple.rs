//! The replies that are not records: VERSION (998) and UPTIME (208). Both are
//! plain commands of the specification, see the crate root for the link.
//!
//! ENCODING (219), MYLISTEXPORT (217, 218) and SENDMSG (294) send no data
//! lines at all, so the reply code is their whole result and there is nothing
//! to decode here.

use crate::error::Error;
use crate::fields::{int, record, text};

/// The server version, which is the text of the data lines.
#[must_use]
pub fn version(lines: &[Vec<String>]) -> String {
    text(lines)
}

/// The server uptime in milliseconds, the one number of the UPTIME reply.
///
/// # Errors
///
/// Returns [`Error::BadData`] when the line has too few fields or a field is not of the expected type.
pub fn uptime(line: &[String]) -> Result<i64, Error> {
    let f = record(line, 1)?;
    int("uptime", &f[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_the_text_of_the_data_lines() {
        let lines = vec![vec!["AniDB UDP Server".to_owned(), "1.2".to_owned()]];
        assert_eq!(version(&lines), "AniDB UDP Server|1.2");
        assert_eq!(version(&[]), "");
    }

    #[test]
    fn uptime_is_a_number_of_milliseconds() {
        assert_eq!(uptime(&["123456".to_owned()]).unwrap(), 123_456);
        assert!(matches!(uptime(&[]), Err(Error::BadData(_))));
        assert!(matches!(
            uptime(&["soon".to_owned()]),
            Err(Error::BadData(_))
        ));
    }
}
