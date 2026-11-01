# anidb-udp

Self-contained, blocking client for the `AniDB` UDP API. The API is specified at <https://wiki.anidb.net/UDP_API_Definition>, and each module names the part of the specification it implements. The crate does not depend on anything else in this workspace.

## Layout

- I/O-free layer: `wire` (request encoding and reply parsing), `crypto` (AES session encryption), `compression` (datagram inflation), `codes` (reply codes), `error` (`Error`, `ErrorKind`), `constants` (server, timing and protocol values).
- Typed replies: `mask` (FILE and ANIME field masks), `file` (FILE), `mylist` (MYLIST), `anime` (ANIME), `responses` (the fixed records of the other commands), `simple` (VERSION and UPTIME).
- Commands: `commands`, one typed method on `Client` for each command.
- Transport: `rate_limiter` and `transport`, one UDP socket with one request in flight.
- Session: `client`, with `Client` and `Client::call`, which sends any request.

## Implemented

**Session.** `Client::connect` logs in, with an ENCRYPT first when an API key is given. `Client::logout`, `Client::ping` and `Client::disconnect` (which also logs out) complete the session commands.

**Commands.** The implemented commands are ANIME, ANIMEDESC, CALENDAR, CHARACTER, CREATOR, EPISODE, FILE, GROUP, GROUPSTATUS, MYLIST (by id, file id, hash or anime), MYLISTADD, MYLISTDEL, MYLISTSTATS, MYLISTEXPORT, RANDOMANIME, SENDMSG, UPDATED, USER, VOTE, ENCODING, VERSION and UPTIME.

**Encryption.** After ENCRYPT, requests and replies use AES-128 in ECB mode with PKCS#7 padding. The key is the MD5 hash of the API key followed by the salt from the ENCRYPT reply. The API key is the UDP key from the profile page, not the password. Encryption stays on until LOGOUT or until the server ends the session.

**Compression.** AUTH always sends `comp=1`. A datagram that starts with two zero bytes is a zlib stream and is inflated before it is parsed.

**AUTH.** Always sends `protover=3`, `nat=1`, `comp=1` and `enc=UTF8`, with the client name and version from `ClientConfig`. A client name that is not 4 to 16 lower case letters is logged as a warning on connect, and the connection goes on (`is_valid_client_name` checks it).

**Masks.** FILE selects its fields with a nine byte mask: bytes 0 to 4 are the file fields (`FileField`), bytes 5 to 8 the anime, episode and group fields (`AnimeField`). ANIME has its own seven byte mask (`AnimeCommandField`). `aid` is always requested, and with no fields given a default set is used. Reserved and retired bits are not in the tables, so they cannot be requested. A reply holds the requested fields in mask order, and the same field set that built the request decodes it (`FileReply::decode`, or the `file_by_hash` method, which takes the sets).

**Typed replies.** Each reply type has a `parse` or `decode` function over the data lines of the reply. A code the table does not list is kept as `ResponseCode::Unknown`. A number that does not parse is `Error::BadData`.

**Rate limiting.** Requests run one at a time. A request may start 5 seconds (`REQUEST_INTERVAL`) after the start of the previous one. The first request of a client does not wait. The limit applies to one `Client`: two clients do not coordinate. A request that gets no reply within 30 seconds (`REQUEST_TIMEOUT`) fails with `Error::Timeout`, and it is not sent again automatically. Replies are matched to their request by a five character tag. Replies with another tag are dropped. 504, 555 and 6xx replies arrive without a tag and are accepted.

**Local port.** `ClientConfig::local_port` binds a fixed local UDP port. The default is `None`, which lets the system pick a port, so a fixed port has to be set. The specification asks for one, since many source ports from one IP get it banned. A busy port falls back to a system picked port with a warning through the logger. With `local_port_fallback` set to `false` a busy port is an error. `Client::local_port()` returns the bound port.

**NAT and keep-alive.** The AUTH reply carries the `ip:port` AniDB sees. `Client::behind_nat()` is `true` when that port differs from the local one, and also before the first AUTH reply and when the reply carries no port. The keep-alive is polled, with no thread (the client is not `Send`). Every command first sends the keep-alive that is due: PING when behind NAT, once the connection has been idle for `ping_interval` (5 minutes by default), and UPTIME otherwise, once it has been idle for `uptime_interval` (30 minutes by default). A program that stays idle between commands calls `Client::keep_alive()` itself, and `Client::keep_alive_due()` says when it is due. Nothing is due without a session, after LOGOUT or after a ban.

**Session loss.** A 501 (LOGIN_FIRST) or 506 (INVALID_SESSION) reply makes the client log in again with the stored credentials and send the request once more. With an API key the login starts over with ENCRYPT and then an encrypted AUTH, so the password is never sent in the clear. AUTH, ENCRYPT and LOGOUT are never retried this way.

**Bans.** A 504 or 555 reply is `Error::Banned`. The client remembers the ban: it sends nothing more, `connect` returns the same error, and `disconnect` sends no LOGOUT. The memory belongs to the `Client`, so a new client does not know about it (see the warning below).

## Not implemented

- The notification and push commands (NOTIFY, NOTIFYLIST, NOTIFYGET, NOTIFYACK, NOTIFICATIONADD, NOTIFICATIONDEL, PUSH, PUSHACK) and the buddy commands (BUDDYADD, BUDDYDEL, BUDDYACCEPT, BUDDYDENY, BUDDYLIST, BUDDYSTATE). No method sends them, because no command of this workspace uses them. Some of their reply codes are in `codes`.
- STATS and TOP. The specification marks both as disabled. Their codes (206, 207) are in `codes`.
- Caching. The specification asks clients to cache what they have asked for, and this client does not cache. The caller does.

## Deliberate deviations

- MYLISTADD by anime without a group sends `generic=1` (`MylistAnimeSpec::group` is `None`). The specification only makes the group optional for edits, but here it is always optional: a file whose hash AniDB does not know yet is added as a generic entry instead of not at all. This also applies to an edit without a group.

## Usage

```rust
use anidb_udp::mask::{AnimeField, AnimeFields, FileField, FileFields};
use anidb_udp::{Client, ClientConfig, Error, ResponseCode};

/// The AniDB file id of a file, or `None` when AniDB does not know its hash.
fn lookup(size: u64, ed2k: &str) -> Result<Option<i64>, Error> {
    let mut config = ClientConfig::new("yourclient", 1);
    config.local_port = Some(40137);
    let mut client = Client::new(config);
    client.set_logger(|message| eprintln!("anidb: {message}"));

    // pass Some(api_key) as the third argument to encrypt the session
    client.connect("user", "password", None)?;

    let file: FileFields = [FileField::Aid, FileField::Size, FileField::Ed2k]
        .into_iter()
        .collect();
    let anime: AnimeFields = [AnimeField::RomanjiName].into_iter().collect();
    let result = match client.file_by_hash(size, ed2k, &file, &anime) {
        Ok(reply) => Ok(Some(reply.fid)),
        Err(error) if error.code() == Some(ResponseCode::NoSuchFile) => Ok(None),
        Err(error) => Err(error),
    };
    client.disconnect();
    result
}
```

Commands take `IdOrName` (an id or a name) where AniDB accepts either, and the typed parameter types (`VoteChoice`, `UpdatedSince`, `MylistFile`, ...) leave no room for invalid combinations. Dropping a `Client` disconnects it as well.

To add a command, build a `Request` with `Request::new(..).param(..)`, pass the reply codes it accepts to `Client::call`, and decode `Response::lines`.

## Tracing

`Client::set_trace` takes a callback that receives one verbose line per event, each prefixed with the milliseconds since the client was created. It is off by default and separate from `set_logger`, which only gets warnings. The first line names the local port. The live tests turn it on (`trace:` on stderr, see `--nocapture`).

```rust
# use anidb_udp::{Client, ClientConfig};
let mut client = Client::new(ClientConfig::new("yourclient", 1));
client.set_trace(|line| eprintln!("trace: {line}"));
```

Traced: every request (command, tag, encrypted or not, datagram length, parameters), every datagram received (length, time since the request, decrypted, compressed, reply code, tag and message), datagrams that cannot be decrypted (their length, and the reply code and tag when the raw bytes are a plain reply, such as the 506 a server sends after it dropped the session), timeouts, encryption key changes, login, session loss and re-login steps, keep-alive and rate-limiter waits.

**No secret is ever traced.** Parameters named `pass`, `s`, or like a key, salt, secret, token or session show as `***`. The session key of a 200 or 201 reply and the salt of a 209 reply show as `***`. Data lines of replies are only counted. Ciphertext is only counted in bytes, and the key is never printed. The line of a failed request has the error kind and reply code, not its text. `tests/fake_trace.rs` checks this against the fake server. The user name and the reply messages are traced as they are.

## Errors

`Error` covers every failure:

- `Reply { code, message }`: a failure reply the command did not expect, such as 320 NO_SUCH_FILE. Every reply code above 305 that the command did not list in its expected codes is an error.
- `Banned { code, message }`: 504 or 555.
- `Server { code, message }`: the server failed, is busy or is out of service (6xx, except the client side codes 670, 672 and 673, which are `Reply`).
- `Timeout { message }`: no reply in time (671).
- `FileAlreadyInMylist { entry }`: 310 on MYLISTADD without `edit`, with the current entry.
- `VoteRefused { code, message, vote }`: 360, 363 or 364 on VOTE.
- `MultipleFilesFound { fids }`: 322, with the file ids.
- `Io`, `BadResponse`, `BadData`, `Decrypt`, `Decompress`: the socket failed, a datagram is not a reply this client can read, the data of a reply does not match its mask, an encrypted reply has a bad length or padding, or a compressed reply is corrupt.
- `InvalidArgument`: an argument is not valid. This is checked before anything is sent.

`Error::code()` gives the reply code behind an error, when there is one. `Error::kind()` gives an `ErrorKind`, and `Error::is_session_fatal()` is `true` for `ErrorKind::SessionFatal`:

- `SessionFatal`: every later request on the client would fail too. This covers bans, server errors, timeouts, socket, bad reply and decryption failures, and session, login and client codes (403, 500 to 503, 505, 506, 509, 519, 598, 670, 672, 673, and codes the table does not know).
- `Request`: only this request failed, and the session is fine. This covers the other reply codes, the 310, 322 and 36x errors, and bad data in one reply.
- `Usage`: the caller made a mistake, and nothing was sent.

`ErrorKind` is `#[non_exhaustive]`, so a `match` needs a wildcard arm. `Error::from_reply(code, message)` classifies a reply code the way the client does.

## Tests

Run the default tests with `cargo test -p anidb-udp`. They are of three kinds:

- Unit tests, inline `#[cfg(test)] mod tests` blocks and sibling `tests.rs` files (`src/anime/tests.rs`, `src/commands/tests.rs`, `src/mask/tests.rs`, `src/mylist/tests.rs`, `src/responses/tests.rs`, `src/wire/tests.rs`).
- Black-box tests that need no server: `tests/file_reply.rs` decodes a recorded FILE reply and `tests/mask_table.rs` checks the mask tables, both against the fixtures in `tests/fixtures/`.
- Fake-server tests, `tests/fake_*.rs` (ANIME, FILE, misc, MYLIST and session commands). The fake server in `tests/common/mod.rs` binds UDP sockets on `127.0.0.1`. A sandbox that forbids binding sockets makes them fail, they do not skip.

The live tests in `tests/live.rs` talk to the real AniDB server. They are `#[ignore]`d, so the default run does not send anything.

- The plain tests need `AKIBA_LIVE_USER`, `AKIBA_LIVE_PASS` and `AKIBA_LIVE_CLIENT`. They never use an API key.
- The encrypted tests (`encrypted_*`) also need `AKIBA_LIVE_APIKEY`, the UDP API key from the profile page.
- A test whose variables are not all set prints which ones are missing and passes without sending anything. Encrypted tests without the API key print that they are skipped.
- `AKIBA_LIVE_LOCAL_PORT` sets the fixed local port of all live tests. The default is 40137. A busy port is an error, the tests never fall back to another port.
- The tests take turns: a process-wide lock is held for each test, even without `--test-threads=1`. Each test ends with a 5 second pause, so the next test starts a request interval after the last request.
- After a ban (504 or 555) the remaining live tests fail at once, without sending anything.

```text
AKIBA_LIVE_USER=.. AKIBA_LIVE_PASS=.. AKIBA_LIVE_CLIENT=.. [AKIBA_LIVE_APIKEY=..] \
  cargo test -p anidb-udp --test live -- --ignored --nocapture --test-threads=1
```

Use `--test-threads=1` as well. A run takes about 1.5 minutes without the API key and about 3.5 minutes with it (estimated from the 5 second spacing, not measured). Run it at most once per 30 minutes.

## Warning: bans

**AniDB bans an IP that sends requests too fast or from many different source ports.** A ban usually lasts 30 minutes. The ban is on the IP, so it applies to every client on that IP, not only to this crate.

- Use one `Client` at a time, on one fixed local port (`ClientConfig::local_port`). Several clients, threads or processes from one IP do not share a rate limit.
- Do not retry in a loop. A dropped request is not resent automatically, but it still counts towards the request rate.
- A new `Client` does not know about an earlier ban. After `Error::Banned`, stop, and wait at least 30 minutes before the next request from this IP.
- The live tests need credentials and network access, run one at a time on one fixed port, and must never run in a loop.
