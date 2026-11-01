# Style guide

The code lives in `crates/` and is built as one Cargo workspace.

- `cargo fmt --all --check`, `cargo clippy --workspace -- -D warnings` and
  `cargo test --workspace` must all be clean.
- `unsafe_code` is forbidden in every crate.
- Error types are `thiserror` enums. Commands return `Result<u8, E>`, and
  `logger::exit_code` logs an error and maps it to exit code 1. `anyhow` is not
  used.
- No `unwrap()` or `expect()` outside tests, except in provably infallible
  spots, which carry a comment saying why.
- `crates/ed2k`, `crates/anidb-udp` and `crates/anidb-http` never depend on
  `akiba-core`, so they can be published on their own.
- Crate layout and dependency direction: the library crates (`ed2k`,
  `anidb-udp`, `anidb-http`) come first, then `akiba-core` (all shared and
  domain code), then one `akiba-cmd-<command>` crate per command, then `akiba`
  (the binary, which only wires the commands together). A crate depends only
  on crates earlier in that order, and command crates never depend on each
  other. `akiba-test-support` is a dev-dependency of the command crates and is
  never published.
- Doc comments are `///`. Inline comments are lower-case `//`. Markers are
  `NOTE:` and `XXX:`.
- User facing output goes through `akiba_core::logger`. The exceptions are
  output the user asked for as data, written to stdout as it is: the bare
  `--version` number, the `configure --dump` JSON and the
  `rename --print-ed2klinks` links.
- Tests follow one layout:
  1. A test of one module's own items stays with that module. If its test
     code is short (under about 100 lines) it is an inline
     `#[cfg(test)] mod tests { .. }`; if it is longer it is the sibling file
     `<module>/tests.rs`, declared with `#[cfg(test)] mod tests;`.
  2. A test that uses only a crate's public API, or needs a fixture file, the
     fake server or the binary, is `crates/<crate>/tests/<topic>.rs`. Shared
     helpers go in `tests/common/mod.rs`, and fixtures in `tests/fixtures/`.
     A fixture lives in the crate that owns the behaviour, and a crate never
     reads fixtures outside its own directory (a crate that needs the same
     data keeps its own copy).
  3. The tests of a command run it in the process through
     `akiba-test-support` (a dev-dependency of the `akiba-cmd-*` crates only,
     never published); `crates/akiba/tests/` keeps the binary and one end to
     end test per command.
  4. Never make an item `pub` to move a test. No test reads anything outside the repository.

  Why: a test next to its code can reach private items, and a long test module
  in its own file keeps the source file readable. Files under `tests/` each
  build as a separate binary and see only the public API, so they suit
  black-box checks and are kept few.
