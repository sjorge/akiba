//! XML output for the NFO types. The elements, their order, attributes and text
//! match the output of akiba 2.x. Whitespace may differ.

use std::io;

use quick_xml::escape::partial_escape;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use super::{jellyfin_id_type, Actor, Episode, TvShow, UniqueId};

/// Writes an XML document one element at a time, indented by two spaces.
struct Xml {
    writer: Writer<Vec<u8>>,
}

impl Xml {
    /// Starts a document with the declaration that xmlbuilder2 writes.
    fn new() -> io::Result<Self> {
        let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
        Ok(Self { writer })
    }

    fn open(&mut self, name: &str, attrs: &[(&str, &str)]) -> io::Result<()> {
        let start = BytesStart::new(name).with_attributes(attrs.iter().copied());
        self.writer.write_event(Event::Start(start))
    }

    fn close(&mut self, name: &str) -> io::Result<()> {
        self.writer.write_event(Event::End(BytesEnd::new(name)))
    }

    /// `<name attrs>text</name>`. Only `&`, `<`, `>` and `\r` are escaped in the
    /// text, as xmlbuilder2 does.
    fn leaf(&mut self, name: &str, attrs: &[(&str, &str)], text: &str) -> io::Result<()> {
        self.open(name, attrs)?;
        self.writer
            .write_event(Event::Text(BytesText::from_escaped(partial_escape(text))))?;
        self.close(name)
    }

    /// A leaf element with text, or nothing when the value is absent.
    fn optional(&mut self, name: &str, value: Option<impl ToString>) -> io::Result<()> {
        match value {
            Some(value) => self.leaf(name, &[], &value.to_string()),
            None => Ok(()),
        }
    }

    fn unique_ids(&mut self, ids: &[UniqueId]) -> io::Result<()> {
        for id in ids {
            let default = if id.default { "true" } else { "false" };
            let attrs = [("type", jellyfin_id_type(&id.kind)), ("default", default)];
            self.leaf("uniqueid", &attrs, &id.id)?;
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.writer.into_inner()
    }
}

/// The full `tvshow.nfo` document.
pub(super) fn tv_show(show: &TvShow) -> io::Result<Vec<u8>> {
    let mut xml = Xml::new()?;
    xml.open("tvshow", &[])?;
    xml.leaf("title", &[], &show.title)?;
    xml.optional("originaltitle", show.original_title.as_deref())?;
    xml.optional("sorttitle", show.sort_title.as_deref())?;
    xml.unique_ids(&show.unique_ids)?;
    xml.optional("premiered", show.premiered.as_deref())?;
    xml.optional("season", show.season)?;
    xml.optional("episode", show.episode)?;
    xml.optional("tagline", show.tagline.as_deref())?;
    xml.optional("plot", show.plot.as_deref())?;
    xml.optional("mpaa", show.mpaa.as_deref())?;
    for tag in &show.tags {
        xml.leaf("tag", &[], tag)?;
    }
    for studio in &show.studios {
        xml.leaf("studio", &[], studio)?;
    }
    for actor in &show.actors {
        actor_element(&mut xml, actor)?;
    }
    xml.close("tvshow")?;
    Ok(xml.finish())
}

fn actor_element(xml: &mut Xml, actor: &Actor) -> io::Result<()> {
    xml.open("actor", &[])?;
    xml.leaf("name", &[], &actor.name)?;
    xml.leaf("role", &[], &actor.role)?;
    xml.optional("order", actor.order)?;
    xml.close("actor")
}

/// The episode document: a fragment with one `<episodedetails>` per episode and
/// no root element. Kodi and Jellyfin expect this form.
pub(super) fn episodes(episodes: &[Episode]) -> io::Result<Vec<u8>> {
    let mut xml = Xml::new()?;
    for episode in episodes {
        xml.open("episodedetails", &[])?;
        xml.leaf("title", &[], &episode.title)?;
        xml.unique_ids(&episode.unique_ids)?;
        xml.optional("originaltitle", episode.original_title.as_deref())?;
        xml.optional("premiered", episode.premiered.as_deref())?;
        xml.optional("season", episode.season)?;
        xml.optional("episode", episode.episode)?;
        xml.optional("plot", episode.plot.as_deref())?;
        xml.close("episodedetails")?;
    }
    Ok(xml.finish())
}
