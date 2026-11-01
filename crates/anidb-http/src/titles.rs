//! anime-titles dump (`anime-titles.xml.gz`) parser.

use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use flate2::read::GzDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::client::{Transport, UreqTransport};
use crate::tls::TlsOptions;
use crate::types::{Error, TitleVariant};

/// Where `AniDB` publishes the titles dump.
pub const TITLES_DUMP_URL: &str = "https://anidb.net/api/anime-titles.xml.gz";

/// Downloads the raw (gzipped) dump.
///
/// # Errors
///
/// Returns [`Error::Tls`] when the CA file cannot be used, and [`Error::Transport`] when the download fails.
pub fn fetch_titles_dump(url: &str, tls: &TlsOptions) -> Result<Vec<u8>, Error> {
    let transport = UreqTransport::new(Duration::from_secs(30), tls)?;
    fetch_titles_dump_with(&transport, url)
}

/// Same as [`fetch_titles_dump`] over any [`Transport`].
///
/// # Errors
///
/// Returns [`Error::Transport`] when the download fails.
pub fn fetch_titles_dump_with(transport: &dyn Transport, url: &str) -> Result<Vec<u8>, Error> {
    Ok(transport.get(url, &[("Accept-Encoding", "gzip")])?)
}

/// Parses the dump (gzip or plain XML) into aid to titles. It keeps only `official` and `main` titles; every aid gets an entry.
///
/// # Errors
///
/// Returns [`Error::Decompress`] when the gzip data is corrupt, and [`Error::Xml`] when the XML is malformed.
pub fn parse_titles(gz_or_xml: &[u8]) -> Result<BTreeMap<u32, Vec<TitleVariant>>, Error> {
    let mut plain = Vec::new();
    let xml: &[u8] = if gz_or_xml.starts_with(&[0x1f, 0x8b]) {
        GzDecoder::new(gz_or_xml)
            .read_to_end(&mut plain)
            .map_err(Error::Decompress)?;
        &plain
    } else {
        gz_or_xml
    };

    let err = |e: &dyn std::fmt::Display| Error::Xml(e.to_string());
    let mut reader = Reader::from_reader(xml);
    let mut map: BTreeMap<u32, Vec<TitleVariant>> = BTreeMap::new();
    let mut aid: Option<u32> = None;
    // (type, lang, text) of the title being read
    let mut cur: Option<(Option<String>, String, String)> = None;

    loop {
        match reader.read_event().map_err(|e| err(&e))? {
            Event::Start(e) => match e.name().as_ref() {
                "anime" => {
                    let mut id = None;
                    for a in e.attributes() {
                        let a = a.map_err(|e| err(&e))?;
                        if a.key.as_ref() == "aid" {
                            id = a
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(|e| err(&e))?
                                .trim()
                                .parse()
                                .ok();
                        }
                    }
                    aid = id;
                    if let Some(id) = id {
                        map.entry(id).or_default();
                    }
                }
                "title" => {
                    let (mut kind, mut lang) = (None, String::new());
                    for a in e.attributes() {
                        let a = a.map_err(|e| err(&e))?;
                        let v = a
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .map_err(|e| err(&e))?
                            .into_owned();
                        match a.key.as_ref() {
                            "type" => kind = Some(v),
                            "xml:lang" => lang = v,
                            _ => {}
                        }
                    }
                    cur = Some((kind, lang, String::new()));
                }
                _ => {}
            },
            Event::Text(t) => {
                if let Some((_, _, text)) = cur.as_mut() {
                    text.push_str(t.as_ref());
                }
            }
            Event::CData(t) => {
                if let Some((_, _, text)) = cur.as_mut() {
                    text.push_str(t.as_ref());
                }
            }
            Event::GeneralRef(r) => {
                if let Some((_, _, text)) = cur.as_mut() {
                    let raw = format!("&{};", r.as_ref() as &str);
                    text.push_str(&quick_xml::escape::unescape(&raw).map_err(|e| err(&e))?);
                }
            }
            Event::End(e) => match e.name().as_ref() {
                "title" => {
                    if let (Some((kind, language, text)), Some(aid)) = (cur.take(), aid) {
                        if matches!(kind.as_deref(), Some("official" | "main")) {
                            // he.decode: decode HTML entities left after XML decoding
                            let title = html_escape::decode_html_entities(&text).into_owned();
                            map.entry(aid).or_default().push(TitleVariant {
                                title,
                                kind,
                                language,
                            });
                        }
                    }
                }
                "anime" => aid = None,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(map)
}
