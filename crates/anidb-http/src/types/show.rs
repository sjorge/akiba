//! Typed anime show. Field names and optionality follow the `AniDB` HTTP API reply,
//! including the extra fields (parent ids, ratings, related, ...).

use serde::{Deserialize, Serialize};

fn is_empty<T>(v: &[T]) -> bool {
    v.is_empty()
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Title {
    #[serde(default)]
    pub title: String,
    /// Absent for episode titles.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Creator {
    #[serde(default)]
    pub id: i64,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    #[serde(default)]
    pub id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<i64>,
    #[serde(default)]
    pub weight: i64,
    #[serde(default)]
    pub local_spoiler: bool,
    #[serde(default)]
    pub global_spoiler: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterType {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Seiyuu {
    #[serde(default)]
    pub id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    #[serde(default)]
    pub id: i64,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub votes: Option<i64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub gender: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_type: Option<CharacterType>,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub seiyuu: Vec<Seiyuu>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub updated_at: String,
    /// A string, to support `S1`, `C1`, ...
    #[serde(default)]
    pub episode_number: String,
    /// Episode type as a number (1 regular, 2 special, ...).
    #[serde(rename = "type", default)]
    pub kind: i64,
    #[serde(default)]
    pub length: i64,
    #[serde(default)]
    pub air_date: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub votes: Option<i64>,
    #[serde(default)]
    pub titles: Vec<Title>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Score and count as the HTTP API returns them (both are raw strings).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rating {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ratings {
    #[serde(default)]
    pub permanent: Rating,
    #[serde(default)]
    pub temporary: Rating,
    #[serde(default)]
    pub review: Rating,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Related {
    #[serde(default)]
    pub id: i64,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Similar {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub approval: i64,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub uid: i64,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEntity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    #[serde(rename = "type", default)]
    pub kind: i64,
    #[serde(default)]
    pub external_entity: Vec<ExternalEntity>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AniDbShow {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub age_restricted: bool,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub episode_count: i64,
    #[serde(default)]
    pub start_date: String,
    #[serde(default)]
    pub end_date: String,
    #[serde(default)]
    pub titles: Vec<Title>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub picture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratings: Option<Ratings>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub related: Vec<Related>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub similar: Vec<Similar>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub recommendations: Vec<Recommendation>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub creators: Vec<Creator>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub resources: Vec<Resource>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub characters: Vec<Character>,
    #[serde(default)]
    pub episodes: Vec<Episode>,
}
