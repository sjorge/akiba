//! Self-contained client for the `AniDB` UDP API.
//!
//! The API is specified on the `AniDB` wiki, at
//! <https://wiki.anidb.net/UDP_API_Definition>. Each module names the part of
//! the specification it implements.
//!
//! Besides the I/O-free layer (request encoding, reply parsing, the AES session
//! crypto, datagram decompression, return codes and errors) the crate has a
//! blocking UDP client with a fixed local port, NAT detection and a poll-based
//! keep-alive, see [`Client`]. It has no dependency on anything akiba specific.
//!
//! Every reply is typed: FILE and MYLIST through the mask tables in [`mask`],
//! [`mod@file`] and [`mod@mylist`]; ANIME through [`mod@anime`]; the fixed records of the
//! other commands in [`responses`]; VERSION and UPTIME in [`simple`]. Each reply
//! type has a `parse` (or `decode`) function over the data lines of the reply.
//!
//! There are no generic field parser objects: each typed reply is a plain function.
//!
//! Every command of the API is a method of [`Client`], see
//! [`commands`] for the typed parameters. The replies of the MYLISTADD and
//! MYLISTDEL writes (210, 211, 310, 311) are decoded there too.

pub mod anime;
pub mod client;
pub mod commands;
pub mod compression;
pub mod crypto;
mod fields;
pub mod file;
pub mod mask;
pub mod mylist;
pub mod rate_limiter;
pub mod responses;
pub mod simple;
mod trace;
pub mod transport;
mod types;
pub mod wire;

pub use types::{codes, constants, error};

pub use client::{is_valid_client_name, Client, ClientConfig, MylistAdd};
pub use codes::ResponseCode;
pub use compression::{decompress, is_compressed};
pub use crypto::Key;
pub use error::{Error, ErrorKind};
pub use types::dateflag::DateFlag;
pub use wire::{make_tag, parse, Request, Response};
