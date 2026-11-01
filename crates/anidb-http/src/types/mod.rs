//! Plain data types of the crate: the show model, the error and the titles
//! result type. The logic lives in the sibling modules.

mod error;
mod show;
mod titles;

pub use error::Error;
pub use show::{
    AniDbShow, Character, CharacterType, Creator, Episode, ExternalEntity, Rating, Ratings,
    Recommendation, Related, Resource, Seiyuu, Similar, Tag, Title,
};
pub use titles::TitleVariant;
