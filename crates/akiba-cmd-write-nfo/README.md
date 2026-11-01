# akiba-cmd-write-nfo

The `write-nfo` command of [`akiba`](../akiba): write `tvshow.nfo`, the poster
and the episode NFO files for a show directory, using the AniDB HTTP API.

The crate exposes `Args`, the `clap::Args` struct with the flags, and `run`,
which returns the process exit code. The application logic lives in
[`akiba-core`](../akiba-core).
