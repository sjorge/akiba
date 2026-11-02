//! Rename template formatter.
//!
//! A template is parsed once into literal and tag parts, with unknown tags
//! rejected up front, and then rendered for each file. Tag names are the
//! serialised [`FileInfo`] field names, including the legacy `quaility` and
//! `orginal_name` spellings that user templates depend on.
//!
//! Every string step renders the same as in akiba 2.x, so existing templates
//! keep working. The first-character modifiers take the first Rust `char`, a
//! whole character even outside the Basic Multilingual Plane. A non-ASCII
//! character is turned into `_` by `sanitize_tag` before the collapse and trim
//! steps, so the rendered output does not depend on how much of it is taken.

use crate::fileinfo::FileInfo;
use crate::num::parse_i64;

/// The accessor of one template field: it reads one value from a [`FileInfo`].
type FieldFn = for<'a> fn(&'a FileInfo) -> Value<'a>;

/// Tag names accepted in `{tag}`, each with the accessor of its [`FileInfo`]
/// field, in the order of the struct. The unit test `tags_match_file_info_fields`
/// keeps the names in step with the struct.
const FIELDS: [(&str, FieldFn); 43] = [
    ("fid", |info| Value::Int(info.fid.into())),
    ("aid", |info| Value::Int(info.aid.into())),
    ("eid", |info| Value::Int(info.eid.into())),
    ("gid", |info| Value::Int(info.gid.into())),
    ("lid", |info| Value::Int(info.lid.into())),
    ("status", |info| Value::Int(info.status.into())),
    ("size", |info| Value::Int(info.size.into())),
    ("ed2k", |info| Value::Text(&info.ed2k)),
    ("md5", |info| Value::Text(&info.md5)),
    ("sha1", |info| Value::Text(&info.sha1)),
    ("crc32", |info| Value::Text(&info.crc32)),
    ("lang_dub", |info| Value::Text(&info.lang_dub)),
    ("lang_sub", |info| Value::Text(&info.lang_sub)),
    ("quaility", |info| Value::Text(&info.quality)),
    ("source", |info| Value::Text(&info.source)),
    ("audio_codec", |info| Value::Text(&info.audio_codec)),
    ("audio_bitrate", |info| Value::Text(&info.audio_bitrate)),
    ("video_codec", |info| Value::Text(&info.video_codec)),
    ("video_bitrate", |info| Value::Text(&info.video_bitrate)),
    ("resolution", |info| Value::Text(&info.resolution)),
    ("filetype", |info| Value::Text(&info.filetype)),
    ("length", |info| Value::Int(info.length.into())),
    ("description", |info| Value::Text(&info.description)),
    ("group", |info| Value::Text(&info.group)),
    ("group_short", |info| Value::Text(&info.group_short)),
    ("episode", |info| Value::Text(&info.episode)),
    ("episode_name", |info| Value::Text(&info.episode_name)),
    ("episode_name_romaji", |info| {
        Value::Text(&info.episode_name_romaji)
    }),
    ("episode_name_kanji", |info| {
        Value::Text(&info.episode_name_kanji)
    }),
    ("episode_total", |info| {
        Value::Int(info.episode_total.into())
    }),
    ("episode_last", |info| Value::Int(info.episode_last.into())),
    ("anime_year", |info| Value::Ints(&info.anime_year)),
    ("anime_type", |info| Value::Text(&info.anime_type)),
    ("anime_name_romaji", |info| {
        Value::Text(&info.anime_name_romaji)
    }),
    ("anime_name_kanji", |info| {
        Value::Text(&info.anime_name_kanji)
    }),
    ("anime_name_english", |info| {
        Value::Text(&info.anime_name_english)
    }),
    ("anime_name_other", |info| {
        Value::Texts(&info.anime_name_other)
    }),
    ("anime_name_short", |info| {
        Value::Texts(&info.anime_name_short)
    }),
    ("anime_synonyms", |info| Value::Texts(&info.anime_synonyms)),
    ("anime_category", |info| Value::Texts(&info.anime_category)),
    ("version", |info| Value::Text(&info.version)),
    ("censored", |info| Value::Text(&info.censored)),
    ("orginal_name", |info| Value::Text(&info.original_name)),
];

/// The tag names for the error message, in the order of [`FIELDS`].
fn known_tags() -> String {
    FIELDS
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Errors from parsing or rendering a template.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    /// A tag that is not a field.
    /// `token` is the full match, including any modifier.
    #[error(
        "The tag {token} is not known, available tags: {}, modifier: upper, lower, lower_first, upper_first, number, or array index as an int.",
        known_tags()
    )]
    UnknownTag { token: String },
    /// The default modifier selected a numeric element of an array, such as
    /// `{anime_year}`, which cannot be sanitised as text. It only happens at render time, so the
    /// same template still parses.
    #[error("the tag {token} selects a number from a list, which cannot be used as text")]
    NumberElement { token: String },
}

/// A parsed rename template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Literal(String),
    Tag(Tag),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Tag {
    /// The full match, such as `{episode:number}`, for error messages.
    token: String,
    /// The index of the field that the tag names, in [`FIELDS`].
    field: usize,
    modifier: Modifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Modifier {
    Upper,
    Lower,
    UpperFirst,
    LowerFirst,
    Number,
    /// Array index. With no modifier it is `0`. `None` means the modifier is not
    /// a leading integer, so an array renders joined with commas.
    Element(Option<usize>),
}

/// One field value. A number renders in decimal and a list renders joined with
/// commas.
enum Value<'a> {
    Int(i128),
    Text(&'a str),
    Ints(&'a [i64]),
    Texts(&'a [String]),
}

impl Value<'_> {
    /// The value as it renders without a modifier: lists are joined with commas.
    fn to_text(&self) -> String {
        match self {
            Self::Int(n) => n.to_string(),
            Self::Text(s) => (*s).to_owned(),
            Self::Ints(items) => items
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(","),
            Self::Texts(items) => items.join(","),
        }
    }
}

impl Template {
    /// Parses and validates a template. Unknown tags are rejected here, not at
    /// render time.
    pub fn parse(template: &str) -> Result<Self, FormatError> {
        let bytes = template.as_bytes();
        let mut parts = Vec::new();
        let mut literal_start = 0;
        let mut pos = 0;
        while pos < bytes.len() {
            let token = (bytes[pos] == b'{')
                .then(|| match_token(template, pos))
                .flatten();
            if let Some((end, name, modifier)) = token {
                if literal_start < pos {
                    parts.push(Part::Literal(template[literal_start..pos].to_owned()));
                }
                let field = FIELDS
                    .iter()
                    .position(|(tag, _)| *tag == name)
                    .ok_or_else(|| FormatError::UnknownTag {
                        token: template[pos..end].to_owned(),
                    })?;
                parts.push(Part::Tag(Tag {
                    token: template[pos..end].to_owned(),
                    field,
                    modifier: parse_modifier(modifier),
                }));
                pos = end;
                literal_start = end;
            } else {
                pos += 1;
            }
        }
        if literal_start < template.len() {
            parts.push(Part::Literal(template[literal_start..].to_owned()));
        }
        Ok(Self { parts })
    }

    /// Renders the template for one file, then applies `sanitize_dir_components`
    /// to the result.
    pub fn render(&self, info: &FileInfo) -> Result<String, FormatError> {
        let mut out = String::new();
        let last = self.parts.len().saturating_sub(1);
        for (index, part) in self.parts.iter().enumerate() {
            match part {
                Part::Literal(text) => {
                    // a tag value ending in dots before the extension would
                    // give "name....ext", drop them
                    if index == last
                        && index > 0
                        && text.starts_with('.')
                        && matches!(self.parts[index - 1], Part::Tag(_))
                    {
                        out.truncate(out.trim_end_matches('.').len());
                    }
                    out.push_str(text);
                }
                Part::Tag(tag) => out.push_str(&tag.render(info)?),
            }
        }
        Ok(sanitize_dir_components(&out))
    }
}

/// Matches `\{(\w+)(?::(\w+))?\}` at `start`, where `template[start]` is `{`.
/// Returns the end offset, the tag name and the modifier. `\w` is ASCII only,
/// so tag names and modifiers never contain other letters.
fn match_token(template: &str, start: usize) -> Option<(usize, &str, Option<&str>)> {
    let bytes = template.as_bytes();
    let name_start = start + 1;
    let mut pos = name_start;
    while pos < bytes.len() && is_word(bytes[pos]) {
        pos += 1;
    }
    if pos == name_start {
        return None;
    }
    let name = &template[name_start..pos];

    let mut modifier = None;
    if pos < bytes.len() && bytes[pos] == b':' {
        let modifier_start = pos + 1;
        let mut end = modifier_start;
        while end < bytes.len() && is_word(bytes[end]) {
            end += 1;
        }
        if end > modifier_start {
            modifier = Some(&template[modifier_start..end]);
            pos = end;
        }
    }

    (pos < bytes.len() && bytes[pos] == b'}').then(|| (pos + 1, name, modifier))
}

const fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn parse_modifier(modifier: Option<&str>) -> Modifier {
    let Some(modifier) = modifier else {
        return Modifier::Element(Some(0));
    };
    match modifier.to_ascii_lowercase().as_str() {
        "upper" => Modifier::Upper,
        "lower" => Modifier::Lower,
        "upper_first" => Modifier::UpperFirst,
        "lower_first" => Modifier::LowerFirst,
        "number" => Modifier::Number,
        _ => Modifier::Element(parse_index(modifier)),
    }
}

/// The array index a modifier names: the whole modifier must be ASCII digits,
/// so `1` is index 1 and `1abc` is not an index. A modifier that is not all
/// digits, or whose number overflows, gives `None`, and then an array renders
/// joined with commas. a template that relied on the leading digits of
/// `1abc` as index 1 renders differently than in akiba 2.x.
fn parse_index(modifier: &str) -> Option<usize> {
    if modifier.is_empty() || !modifier.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    modifier.parse().ok()
}

impl Tag {
    fn render(&self, info: &FileInfo) -> Result<String, FormatError> {
        // `field` comes from `position` over FIELDS in `parse`, so it is in range.
        let value = (FIELDS[self.field].1)(info);
        Ok(match self.modifier {
            Modifier::Upper => sanitize_tag(&value.to_text().to_uppercase()),
            Modifier::Lower => sanitize_tag(&value.to_text().to_lowercase()),
            Modifier::UpperFirst => sanitize_tag(&first_char(&value.to_text().to_uppercase())),
            Modifier::LowerFirst => sanitize_tag(&first_char(&value.to_text().to_lowercase())),
            Modifier::Number => number(&value),
            Modifier::Element(index) => match (&value, index) {
                (Value::Texts(items), Some(index)) if index < items.len() => {
                    sanitize_tag(&items[index])
                }
                (Value::Ints(items), Some(index)) if index < items.len() => {
                    return Err(FormatError::NumberElement {
                        token: self.token.clone(),
                    });
                }
                _ => sanitize_tag(&value.to_text()),
            },
        })
    }
}

/// The first character of `text`, or an empty string. See the module docs for
/// why this is not the first UTF-16 unit.
fn first_char(text: &str) -> String {
    text.chars().next().map(String::from).unwrap_or_default()
}

/// The `number` modifier. A whole number loses its leading zeros, and an `a-b`
/// range of whole numbers keeps both sides. Anything else, such as a value with
/// a type prefix (`S1`, `S01-S03`) or with junk after the digits, falls back to
/// the sanitised value as written, so a special never turns into a normal episode.
fn number(value: &Value<'_>) -> String {
    let Value::Text(text) = value else {
        return sanitize_tag(&value.to_text());
    };
    let parts: Vec<&str> = text.split('-').collect();
    if let [start, stop] = parts[..] {
        if let (Some(start), Some(stop)) = (parse_i64(start), parse_i64(stop)) {
            return format!("{start}-{stop}");
        }
    } else if let Some(number) = parse_i64(text) {
        return number.to_string();
    }
    sanitize_tag(text)
}

/// The `sanitizeTag` chain, in a fixed order. Only ASCII letters, digits
/// and the punctuation `-&!',.~+` plus space and parentheses survive, and `/` is
/// dropped.
#[must_use]
pub fn sanitize_tag(value: &str) -> String {
    let mapped: String = value
        .chars()
        .map(|c| if c == '`' { '\'' } else { c })
        .filter(|&c| c != '/')
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-&!',.~+ ()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();

    // collapse runs of `_`, then strip one leading and one trailing `_`
    let mut collapsed = String::with_capacity(mapped.len());
    for c in mapped.chars() {
        if !(c == '_' && collapsed.ends_with('_')) {
            collapsed.push(c);
        }
    }
    collapsed
        .trim_matches('_')
        .replace("_.", ".")
        .replace("_!", "!")
        .replace("_-.", "-")
        .replace("_ ", " ")
        .replace(" _", " ")
}

/// Sanitises the directory components of a rendered path. It splits on `/` and drops one trailing
/// `.` from each component, except a component equal to the basename. The
/// comparison is by value and uses the POSIX basename of the whole string
/// (so rendered paths stay the same as in akiba 2.x), which ignores trailing
/// slashes.
#[must_use]
pub fn sanitize_dir_components(rendered: &str) -> String {
    let base = basename(rendered);
    rendered
        .split('/')
        .map(|chunk| match chunk.strip_suffix('.') {
            Some(stripped) if chunk != base => stripped,
            _ => chunk,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The last `/` separated part of `path`, ignoring trailing slashes. For `a/b/`
/// it is `b`.
fn basename(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
