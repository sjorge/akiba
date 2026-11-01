//! The plain data types of the crate: return codes, the error, constants, the
//! typed replies and command parameters, and the mask field tables. The code
//! that builds, parses and sends is in the sibling modules, which re-export the
//! types so that the public paths (`anidb_udp::mask::FileField`,
//! `anidb_udp::responses::Group`, ...) stay as they were.

pub(crate) mod anime;
pub mod codes;
pub(crate) mod commands;
pub mod constants;
pub(crate) mod dateflag;
pub mod error;
pub(crate) mod fields;
pub(crate) mod file;
pub(crate) mod mask;
pub(crate) mod mylist;
pub(crate) mod responses;
