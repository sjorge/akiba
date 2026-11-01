//! Result types of the anime-titles dump.

use serde::{Deserialize, Serialize};

/// One title of an anime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleVariant {
    pub title: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub language: String,
}
