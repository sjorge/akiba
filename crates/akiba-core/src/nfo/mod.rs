//! Jellyfin NFO files: `tvshow.nfo` for a show, and `<episode>.nfo` for the
//! episodes of one media file.
//!
//! This is not a full NFO implementation: only
//! the fields akiba writes are modelled. The anime mapping lives in [`anime`].

pub mod anime;
mod xml;

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::fs::{write_atomic, write_atomic_at};

/// Mode of `tvshow.nfo`, the episode NFOs and `poster.jpg`. Jellyfin may run as
/// another user, so every file the NFO step writes stays world readable.
const FILE_MODE: u32 = 0o644;
const TV_SHOW_FILE: &str = "tvshow.nfo";
const POSTER_FILE: &str = "poster.jpg";

/// An external id, as in `<uniqueid type="...">`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UniqueId {
    pub id: String,
    /// Akiba's name for the provider, such as `anidb`. Written as Jellyfin's
    /// name (`AniDB`) where one is known.
    pub kind: String,
    /// Exactly one id per show or episode must be the default.
    pub default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Actor {
    pub name: String,
    pub role: String,
    pub order: Option<i64>,
}

/// The fields of `tvshow.nfo` that akiba writes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TvShow {
    pub unique_ids: Vec<UniqueId>,
    pub title: String,
    pub original_title: Option<String>,
    pub sort_title: Option<String>,
    /// `YYYY-MM-DD`.
    pub premiered: Option<String>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub tagline: Option<String>,
    pub plot: Option<String>,
    pub mpaa: Option<String>,
    pub tags: Vec<String>,
    pub studios: Vec<String>,
    pub actors: Vec<Actor>,
    /// Modification time of `tvshow.nfo`, the first airing date. `None` leaves
    /// the time as written.
    pub modified: Option<SystemTime>,
}

/// One `<episodedetails>` element.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Episode {
    pub unique_ids: Vec<UniqueId>,
    pub title: String,
    pub original_title: Option<String>,
    /// `YYYY-MM-DD`.
    pub premiered: Option<String>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub plot: Option<String>,
    /// Modification time of the NFO, the air date. `None` leaves the time as
    /// written. For a file with several episodes the first one counts.
    pub modified: Option<SystemTime>,
}

#[derive(Debug, thiserror::Error)]
pub enum NfoError {
    #[error("The path '{}' does not exist or is not a directory!", .0.display())]
    NotADirectory(PathBuf),
    #[error("The path '{}' does not exist or is not a file!", .0.display())]
    NotAFile(PathBuf),
    #[error("Cannot write invalid data to \"{}\" file!", .0.display())]
    Invalid(PathBuf),
    #[error("{}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The anime has no titles, so there is no `<title>` to write.
    #[error("AniDB anime {aid} has no titles, refusing to write its NFO without a title")]
    NoTitles { aid: i64 },
}

fn io_error(path: &Path, source: io::Error) -> NfoError {
    NfoError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Maps an akiba id type to the name Jellyfin expects.
fn jellyfin_id_type(kind: &str) -> &str {
    match kind {
        "anidb" => "AniDB",
        "anilist" => "AniList",
        other => other,
    }
}

/// Exactly one default id is required. A single id is made the default.
fn unique_ids_are_valid(ids: &mut [UniqueId]) -> bool {
    if ids.is_empty() {
        return false;
    }
    if let [only] = ids {
        only.default = true;
    }
    ids.iter().filter(|id| id.default).count() == 1
}

fn premiered_is_valid(premiered: Option<&str>) -> bool {
    match premiered {
        None | Some("") => true,
        Some(date) => is_iso_date(date),
    }
}

/// `YYYY-MM-DD` with a month of 1 to 12 and a day of 1 to 31. Only this exact
/// shape is accepted; the builders never produce another.
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| b[range].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let month = u32::from(b[5] - b'0') * 10 + u32::from(b[6] - b'0');
    let day = u32::from(b[8] - b'0') * 10 + u32::from(b[9] - b'0');
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// A `tvshow.nfo` for a directory.
#[derive(Debug, Clone)]
pub struct TvShowNfo {
    pub tv_show: TvShow,
    dir: PathBuf,
}

impl TvShowNfo {
    pub fn new(tv_show: TvShow, dir: impl AsRef<Path>) -> Result<Self, NfoError> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Err(NfoError::NotADirectory(dir.to_path_buf()));
        }
        Ok(Self {
            tv_show,
            dir: dir.to_path_buf(),
        })
    }

    /// Checks the data and normalises the default id.
    pub fn is_valid(&mut self) -> bool {
        let show = &mut self.tv_show;
        unique_ids_are_valid(&mut show.unique_ids) && premiered_is_valid(show.premiered.as_deref())
    }

    /// Whether [`write`](Self::write) would store a poster with these arguments.
    /// The caller checks this before it downloads the poster, so an existing
    /// `poster.jpg` is not fetched again.
    #[must_use]
    pub fn needs_poster(&self, overwrite: bool) -> bool {
        overwrite || !self.dir.join(POSTER_FILE).exists()
    }

    /// Writes `tvshow.nfo` unless it exists and `overwrite` is false. Then writes
    /// `poster` to `poster.jpg`, if given. The poster is kept when it exists and
    /// `overwrite` is false. The bytes are fetched by the caller.
    pub fn write(&mut self, overwrite: bool, poster: Option<&[u8]>) -> Result<(), NfoError> {
        let nfo_path = self.dir.join(TV_SHOW_FILE);
        if !self.is_valid() {
            return Err(NfoError::Invalid(nfo_path));
        }

        if overwrite || !nfo_path.exists() {
            let body = xml::tv_show(&self.tv_show).map_err(|e| io_error(&nfo_path, e))?;
            write_atomic_at(&nfo_path, &body, FILE_MODE, self.tv_show.modified)
                .map_err(|e| io_error(&nfo_path, e))?;
        }

        if let Some(bytes) = poster.filter(|_| self.needs_poster(overwrite)) {
            let poster_path = self.dir.join(POSTER_FILE);
            write_atomic(&poster_path, bytes, FILE_MODE).map_err(|e| io_error(&poster_path, e))?;
        }
        Ok(())
    }
}

/// The episode NFO of a media file. It is written next to the file, with the
/// extension replaced by `.nfo`, and holds one `<episodedetails>` per episode.
#[derive(Debug, Clone)]
pub struct EpisodeNfo {
    pub episodes: Vec<Episode>,
    path: PathBuf,
}

impl EpisodeNfo {
    pub fn new(episodes: Vec<Episode>, path: impl AsRef<Path>) -> Result<Self, NfoError> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(NfoError::NotAFile(path.to_path_buf()));
        }
        Ok(Self {
            episodes,
            path: path.to_path_buf(),
        })
    }

    /// Checks every episode and normalises its default id. An empty list is
    /// invalid.
    pub fn is_valid(&mut self) -> bool {
        let mut valid = true;
        for episode in &mut self.episodes {
            valid &= unique_ids_are_valid(&mut episode.unique_ids)
                && premiered_is_valid(episode.premiered.as_deref());
        }
        valid && !self.episodes.is_empty()
    }

    /// Writes the `.nfo` unless it exists and `overwrite` is false.
    pub fn write(&mut self, overwrite: bool) -> Result<(), NfoError> {
        let nfo_path = self.path.with_extension("nfo");
        if !self.is_valid() {
            return Err(NfoError::Invalid(nfo_path));
        }
        // a kept NFO is left alone, content and time: only `--force` rewrites an
        // NFO and gives it the AniDB date
        if !overwrite && nfo_path.exists() {
            return Ok(());
        }
        let modified = self.episodes.first().and_then(|episode| episode.modified);
        let body = xml::episodes(&self.episodes).map_err(|e| io_error(&nfo_path, e))?;
        write_atomic_at(&nfo_path, &body, FILE_MODE, modified).map_err(|e| io_error(&nfo_path, e))
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::fmt::Write as _;

    use quick_xml::events::Event;
    use quick_xml::{Reader, XmlVersion};

    /// The XML as a list of events. Attribute values and text are unescaped.
    /// Whitespace-only text between elements is dropped, so formatting
    /// differences do not matter, but the content of text nodes is exact.
    pub(crate) fn events(xml: &[u8]) -> Vec<String> {
        let mut reader = Reader::from_reader(xml);
        let mut out = Vec::new();
        // text and entity references arrive as separate events, so they are
        // joined before they are compared
        let mut text = String::new();
        loop {
            let event = reader.read_event().expect("well-formed XML");
            match &event {
                Event::Text(raw) => {
                    let raw = raw.xml10_content();
                    text.push_str(&quick_xml::escape::unescape(&raw).expect("text"));
                    continue;
                }
                Event::GeneralRef(raw) => {
                    let raw = format!("&{};", raw.as_ref() as &str);
                    text.push_str(&quick_xml::escape::unescape(&raw).expect("reference"));
                    continue;
                }
                _ if !text.is_empty() => {
                    let joined = std::mem::take(&mut text);
                    if !joined.trim().is_empty() {
                        out.push(format!("text {joined:?}"));
                    }
                }
                _ => {}
            }
            match event {
                Event::Eof => break,
                Event::Start(start) => {
                    let mut line = format!("<{}", start.name().as_ref() as &str);
                    for attr in start.attributes() {
                        let attr = attr.expect("attribute");
                        let value = attr
                            .normalized_value(XmlVersion::Implicit1_0)
                            .expect("attribute value");
                        let key = attr.key.as_ref() as &str;
                        let _ = write!(line, " {key}={value:?}");
                    }
                    line.push('>');
                    out.push(line);
                }
                Event::End(end) => {
                    out.push(format!("</{}>", end.name().as_ref() as &str));
                }
                other @ (Event::Text(_) | Event::GeneralRef(_)) => {
                    unreachable!("handled above: {other:?}")
                }
                other => out.push(format!("{other:?}")),
            }
        }
        out
    }

    /// A file from `tests/fixtures/nfo`.
    pub(crate) fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/tests/fixtures/nfo/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }
}

#[cfg(test)]
mod tests;
