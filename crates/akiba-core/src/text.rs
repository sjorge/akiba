//! Text helpers: natural ordering of file names, and a fuzzy distance that
//! compares letters the way a locale aware collator does (accents and case
//! ignored).

use std::cmp::Ordering;

use unicode_normalization::char::{decompose_compatible, is_combining_mark};

/// Natural order of two strings: digit runs compare as numbers, so `2` sorts
/// before `10` and `001` before `010`. Episode files and titles are sorted with
/// it. The comparison is the `alphanumeric-sort` crate's, case sensitive. Numbers
/// with leading zeros or a fraction have a documented order (see the expected
/// orders in the tests), which only decides the order files are processed in.
#[must_use]
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    alphanumeric_sort::compare_str(a, b)
}

/// A collation token: one per UTF-16 code unit of the input, as the collator
/// sees it. Ignorable units share [`IGNORABLE`]. A lone surrogate keeps its code
/// unit value, which is never a valid `char`.
pub type Token = u32;

/// The token of a unit that has no primary weight (combining marks, controls,
/// zero-width characters). Two such units compare equal, as in the collator.
pub const IGNORABLE: Token = u32::MAX;

/// Folds `s` into one token per UTF-16 code unit.
///
/// The fuzzy distance compares each UTF-16 code unit on its own, and each
/// unit costs one edit. It compares units at the base level of a
/// locale aware collator, which ignores case, accents and width, and treats katakana and hiragana
/// alike. So a unit folds to its compatibility decomposition, without combining
/// marks, lowercased and with katakana mapped to hiragana. A unit that folds to
/// one character gives that character. A unit that folds to nothing is
/// [`IGNORABLE`]. A unit that folds to several characters, such as a Hangul
/// syllable or a ligature, keeps its own lowercased form.
#[must_use]
pub fn fold(s: &str) -> Vec<Token> {
    let mut out = Vec::with_capacity(s.len());
    for unit in s.encode_utf16() {
        let Some(c) = char::from_u32(u32::from(unit)) else {
            out.push(u32::from(unit));
            continue;
        };
        if is_ignorable(c) {
            out.push(IGNORABLE);
            continue;
        }

        let mut count = 0;
        let mut first = None;
        decompose_compatible(c, |d| {
            if is_combining_mark(d) {
                return;
            }
            for lower in kana_to_hiragana(d).to_lowercase() {
                count += 1;
                first.get_or_insert(lower);
            }
        });
        out.push(match (count, first) {
            (1, Some(lower)) => u32::from(lower),
            (0, _) => IGNORABLE,
            _ => u32::from(c.to_lowercase().next().unwrap_or(c)),
        });
    }
    out
}

/// Controls, soft hyphen and the format characters that DUCET leaves without
/// a primary weight. TAB, LF, VT, FF, CR and NEL are not in this set, because
/// they keep one.
fn is_ignorable(c: char) -> bool {
    matches!(
        c,
        '\u{0}'..='\u{8}'
            | '\u{E}'..='\u{1F}'
            | '\u{7F}'..='\u{84}'
            | '\u{86}'..='\u{9F}'
            | '\u{AD}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{FEFF}'
    )
}

fn kana_to_hiragana(c: char) -> char {
    match u32::from(c) {
        0x30A1..=0x30F6 => char::from_u32(u32::from(c) - 0x60).unwrap_or(c),
        _ => c,
    }
}

/// Edit distance between two folded strings, with every edit costing 1.
#[must_use]
pub fn levenshtein_folded(a: &[Token], b: &[Token]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Edit distance between two titles, used by the titles fuzzy match and the
/// `AniList` match. Both strings are folded with [`fold`] first, so case and diacritics do not
/// count.
#[must_use]
pub fn levenshtein(a: &str, b: &str) -> usize {
    levenshtein_folded(&fold(a), &fold(b))
}

#[cfg(test)]
mod tests;
