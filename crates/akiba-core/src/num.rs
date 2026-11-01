//! Strict integer parsing shared by the argument parsers, the formatter, the
//! NFO writer and the renamer. The whole text (minus surrounding whitespace)
//! must be one integer: an optional sign, then ASCII digits. Junk after the
//! digits, an empty string, a lone sign, a decimal point or an exponent give
//! `None`, and so does a value that does not fit.

/// An integer such as `12`, `+12`, `-7` or ` 12 `. The surrounding whitespace
/// is what [`str::trim`] removes.
#[must_use]
pub fn parse_i64(text: &str) -> Option<i64> {
    text.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_i64_takes_the_whole_text() {
        assert_eq!(parse_i64("12"), Some(12));
        assert_eq!(parse_i64("+12"), Some(12));
        assert_eq!(parse_i64(" 12 "), Some(12));
        assert_eq!(parse_i64("-7"), Some(-7));
        assert_eq!(parse_i64("007"), Some(7));
        assert_eq!(parse_i64("0"), Some(0));
    }

    #[test]
    fn parse_i64_rejects_everything_else() {
        for text in [
            "",
            " ",
            "-",
            "+",
            "12x",
            "x12",
            "1 2",
            "1.5",
            "1e3",
            "0x10",
            "--1",
            "+-1",
            "NaN",
            "inf",
            "１２",
            "9223372036854775808",
        ] {
            assert_eq!(parse_i64(text), None, "{text:?}");
        }
        assert_eq!(parse_i64("9223372036854775807"), Some(i64::MAX));
    }

    #[test]
    fn parse_i64_trims_unicode_whitespace() {
        assert_eq!(parse_i64("\u{a0}12\u{85}"), Some(12));
        assert_eq!(parse_i64("\t-3\n"), Some(-3));
    }
}
