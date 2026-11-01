//! `AniDB` HTTP API anime XML parser and anime-titles dump parser.
//!
//! The serialised [`AniDbShow`] keeps the exact camelCase field names that the
//! legacy `anidb/<aid>.aid.toml` caches use.

mod client;
mod poster;
mod titles;
pub mod tls;
mod types;
mod xml;

pub use client::{HttpApi, Transport, TransportError, UreqTransport, DEFAULT_BASE_URL};
pub use poster::{fetch_poster, poster_url, POSTER_BASE_URL};
pub use titles::{fetch_titles_dump, fetch_titles_dump_with, parse_titles, TITLES_DUMP_URL};
pub use tls::{TlsError, TlsOptions};
pub use types::{
    AniDbShow, Character, CharacterType, Creator, Episode, Error, ExternalEntity, Rating, Ratings,
    Recommendation, Related, Resource, Seiyuu, Similar, Tag, Title, TitleVariant,
};
pub use xml::parse_anime;
