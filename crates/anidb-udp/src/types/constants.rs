//! Server, timing and protocol constants of the `AniDB` UDP API.
//!
//! The values come from the connection rules of the API specification (see the
//! crate root for the link): the server and port, the protocol version, the
//! flood protection and the keep-alive intervals.

use std::time::Duration;

/// Host of the UDP API, `api.anidb.net`. The spec asks clients to read it from
/// configuration, so the transport takes it as an option.
pub const HOST: &str = "api.anidb.net";

/// Port of the UDP API, 9000 over UDP.
pub const PORT: u16 = 9000;

/// Version of the API spec this client was written against, sent as `protover`
/// in AUTH. A client may send nothing but the version of the spec its author
/// used. The server compares it with its own and refuses clients whose version
/// is too old (503).
pub const PROTOCOL_VERSION: u16 = 3;

/// Flood protection: the short term limit is 0.5 packets per second after the
/// first five, the long term limit one packet per four seconds. Requests run
/// one at a time with this spacing, 25 percent above the long term limit. Do
/// not lower this.
pub const REQUEST_INTERVAL: Duration = Duration::from_secs(5);

/// No reply within this time means the packet or the reply was dropped, the
/// server is down or the IP is banned (usually for 30 minutes). Dropped packets
/// still count towards the packet rate, so there is no automatic resend.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Behind NAT: idle time after which a PING keeps the NAT mapping alive. The
/// spec asks for pings in regular intervals, but not more often than needed to
/// keep the mapping. It also warns that too many different source ports from
/// one IP within about an hour get the IP banned, and a new mapping means a new
/// port, so the mapping is kept alive instead.
pub const PING_INTERVAL: Duration = Duration::from_mins(5);

/// Without NAT: idle time after which UPTIME keeps the session open. The server
/// drops a session after 35 minutes without data, and the spec asks for an
/// UPTIME about every 30 minutes.
pub const UPTIME_INTERVAL: Duration = Duration::from_mins(30);
