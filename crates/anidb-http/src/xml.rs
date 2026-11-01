//! quick-xml to [`AniDbShow`] mapping.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::types::Error;
use crate::types::{
    AniDbShow, Character, CharacterType, Creator, Episode, ExternalEntity, Rating, Ratings,
    Recommendation, Related, Resource, Seiyuu, Similar, Tag, Title,
};

/// Minimal DOM node: name, attributes, text and child nodes.
#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// Text of the node; `None` when empty and the node carries attributes
    /// (an element such as `<a x="1"/>` has no text, while `<a/>` has empty text).
    fn text(&self) -> Option<&str> {
        if self.text.is_empty() && !self.attrs.is_empty() {
            None
        } else {
            Some(&self.text)
        }
    }

    fn child_text(&self, name: &str) -> Option<String> {
        self.child(name).and_then(Node::text).map(str::to_owned)
    }

    fn child_string(&self, name: &str) -> String {
        self.child_text(name).unwrap_or_default()
    }
}

/// A whole integer (optional sign, ASCII digits), or `None`. Surrounding
/// whitespace is ignored; junk after the digits is not accepted.
fn whole_int(s: &str) -> Option<i64> {
    s.trim().parse().ok()
}

/// A finite decimal number, or `None` (`NaN`, `inf` and junk are rejected).
fn parse_float(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|f| f.is_finite())
}

fn attr_int(n: &Node, key: &str) -> Option<i64> {
    n.attr(key).and_then(whole_int)
}

fn attr_int_or0(n: &Node, key: &str) -> i64 {
    attr_int(n, key).unwrap_or_default()
}

fn attr_bool(n: &Node, key: &str) -> bool {
    n.attr(key) == Some("true")
}

fn attr_string(n: &Node, key: &str) -> String {
    n.attr(key).unwrap_or_default().to_owned()
}

fn child_int(n: &Node, name: &str) -> i64 {
    n.child_text(name)
        .and_then(|t| whole_int(&t))
        .unwrap_or_default()
}

fn parse_dom(xml: &[u8]) -> Result<Node, Error> {
    let mut reader = Reader::from_reader(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let err = |e: &dyn std::fmt::Display| Error::Xml(e.to_string());

    loop {
        match reader.read_event().map_err(|e| err(&e))? {
            Event::Start(e) => stack.push(open_node(&e)?),
            Event::Empty(e) => {
                stack.push(open_node(&e)?);
                close(&mut stack, &mut root);
            }
            Event::End(_) => close(&mut stack, &mut root),
            Event::Text(t) => {
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(t.as_ref());
                }
            }
            Event::CData(t) => {
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(t.as_ref());
                }
            }
            Event::GeneralRef(r) => {
                let raw = format!("&{};", r.as_ref() as &str);
                let s = quick_xml::escape::unescape(&raw).map_err(|e| err(&e))?;
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&s);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.ok_or_else(|| Error::Xml("no root element".into()))
}

fn open_node(e: &quick_xml::events::BytesStart<'_>) -> Result<Node, Error> {
    let mut node = Node {
        name: e.name().as_ref().to_owned(),
        ..Node::default()
    };
    for a in e.attributes() {
        let a = a.map_err(|e| Error::Xml(e.to_string()))?;
        node.attrs.push((
            a.key.as_ref().to_owned(),
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|e| Error::Xml(e.to_string()))?
                .into_owned(),
        ));
    }
    Ok(node)
}

fn close(stack: &mut Vec<Node>, root: &mut Option<Node>) {
    if let Some(mut node) = stack.pop() {
        // whitespace-only text is dropped, so indentation between elements is not text
        if node.text.trim().is_empty() {
            node.text.clear();
        }
        match stack.last_mut() {
            Some(parent) => parent.children.push(node),
            None => *root = Some(node),
        }
    }
}

fn map_title(n: &Node) -> Title {
    Title {
        title: n.text().unwrap_or_default().to_owned(),
        language: attr_string(n, "xml:lang"),
        kind: n.attr("type").filter(|t| !t.is_empty()).map(str::to_owned),
    }
}

fn map_list<T>(n: &Node, container: &str, item: &str, f: impl Fn(&Node) -> T) -> Vec<T> {
    n.child(container)
        .map(|c| c.children_named(item).map(&f).collect())
        .unwrap_or_default()
}

fn map_tag(n: &Node) -> Tag {
    Tag {
        id: attr_int_or0(n, "id"),
        parent_id: attr_int(n, "parentid"),
        weight: attr_int_or0(n, "weight"),
        local_spoiler: attr_bool(n, "localspoiler"),
        global_spoiler: attr_bool(n, "globalspoiler"),
        name: n.child_string("name"),
        description: n.child_string("description"),
        updated_at: attr_string(n, "update"),
        picture_url: n.child_text("picurl"),
    }
}

fn map_rating(n: Option<&Node>) -> Rating {
    Rating {
        score: n.and_then(Node::text).map(str::to_owned),
        count: n.and_then(|n| n.attr("count")).map(str::to_owned),
    }
}

fn map_character(n: &Node) -> Character {
    let rating = n.child("rating");
    Character {
        id: attr_int_or0(n, "id"),
        kind: attr_string(n, "type"),
        updated_at: attr_string(n, "update"),
        rating: rating.and_then(|r| r.text()).and_then(parse_float),
        votes: rating.and_then(|r| attr_int(r, "votes")),
        name: n.child_string("name"),
        gender: n.child_string("gender"),
        character_type: n.child("charactertype").map(|c| CharacterType {
            id: attr_int_or0(c, "id"),
            name: c.text().unwrap_or_default().to_owned(),
        }),
        description: n.child_string("description"),
        picture: n.child_text("picture"),
        seiyuu: n
            .children_named("seiyuu")
            .map(|s| Seiyuu {
                id: attr_int_or0(s, "id"),
                picture: s.attr("picture").map(str::to_owned),
                name: s.text().unwrap_or_default().to_owned(),
            })
            .collect(),
    }
}

fn map_episode(n: &Node) -> Episode {
    let epno = n.child("epno");
    let rating = n.child("rating");
    Episode {
        id: attr_int_or0(n, "id"),
        updated_at: attr_string(n, "update"),
        episode_number: epno.and_then(Node::text).unwrap_or_default().to_owned(),
        kind: epno.map_or(0, |e| attr_int_or0(e, "type")),
        length: child_int(n, "length"),
        air_date: n.child_string("airdate"),
        rating: rating.and_then(|r| r.text()).and_then(parse_float),
        votes: rating.and_then(|r| attr_int(r, "votes")),
        titles: n.children_named("title").map(map_title).collect(),
        summary: n.child_text("summary").filter(|s| !s.is_empty()),
    }
}

fn map_resource(n: &Node) -> Resource {
    Resource {
        kind: attr_int_or0(n, "type"),
        external_entity: n
            .children_named("externalentity")
            .map(|e| ExternalEntity {
                ids: {
                    let ids: Vec<String> = e
                        .children_named("identifier")
                        .map(|i| i.text().unwrap_or_default().to_owned())
                        .collect();
                    (!ids.is_empty()).then_some(ids)
                },
                url: e.child_text("url"),
            })
            .collect(),
    }
}

/// Maps an `<anime>` HTTP API reply onto [`AniDbShow`].
///
/// # Errors
///
/// Returns [`Error::Xml`] when the XML is malformed or lacks a required element, and the matching [`Error`] variant when the reply is an `<error>` element.
pub fn parse_anime(xml: &[u8]) -> Result<AniDbShow, Error> {
    let n = parse_dom(xml)?;
    if n.name != "anime" {
        return Err(Error::Xml(format!("unexpected root element <{}>", n.name)));
    }

    let ratings = n.child("ratings").map(|r| Ratings {
        permanent: map_rating(r.child("permanent")),
        temporary: map_rating(r.child("temporary")),
        review: map_rating(r.child("review")),
    });

    Ok(AniDbShow {
        id: attr_int_or0(&n, "id"),
        age_restricted: attr_bool(&n, "restricted"),
        kind: n.child_string("type"),
        episode_count: child_int(&n, "episodecount"),
        start_date: n.child_string("startdate"),
        end_date: n.child_string("enddate"),
        titles: map_list(&n, "titles", "title", map_title),
        description: n.child_string("description"),
        picture: n.child_string("picture"),
        ratings,
        related: map_list(&n, "relatedanime", "anime", |r| Related {
            id: attr_int_or0(r, "id"),
            kind: attr_string(r, "type"),
            name: r.text().unwrap_or_default().to_owned(),
        }),
        similar: map_list(&n, "similaranime", "anime", |s| Similar {
            id: attr_int_or0(s, "id"),
            approval: attr_int_or0(s, "approval"),
            total: attr_int_or0(s, "total"),
            name: s.text().unwrap_or_default().to_owned(),
        }),
        recommendations: map_list(&n, "recommendations", "recommendation", |r| {
            Recommendation {
                kind: attr_string(r, "type"),
                uid: attr_int_or0(r, "uid"),
                content: r.text().unwrap_or_default().to_owned(),
            }
        }),
        url: n.child_string("url"),
        creators: map_list(&n, "creators", "name", |c| Creator {
            id: attr_int_or0(c, "id"),
            kind: attr_string(c, "type"),
            name: c.text().unwrap_or_default().to_owned(),
        }),
        resources: map_list(&n, "resources", "resource", map_resource),
        tags: map_list(&n, "tags", "tag", map_tag),
        characters: map_list(&n, "characters", "character", map_character),
        episodes: map_list(&n, "episodes", "episode", map_episode),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_int_takes_the_whole_text() {
        assert_eq!(whole_int("12"), Some(12));
        assert_eq!(whole_int(" +12 "), Some(12));
        assert_eq!(whole_int("-3"), Some(-3));
        for bad in ["", "12x", "1.5", "1e3", "-", "NaN"] {
            assert_eq!(whole_int(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn parse_float_is_finite_and_whole() {
        assert_eq!(parse_float("8.25"), Some(8.25));
        assert_eq!(parse_float(" 8 "), Some(8.0));
        for bad in ["", "8.25x", "NaN", "inf", "-infinity", "1e999"] {
            assert_eq!(parse_float(bad), None, "{bad:?}");
        }
    }
}
