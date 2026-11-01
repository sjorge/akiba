# akiba-cmd-idmap

The `idmap` command of [`akiba`](../akiba): add, remove and show the local
AniDB to AniList and TheMovieDB id mapping entries.

The crate exposes `Args`, the `clap::Args` struct with the flags, and `run`,
which returns the process exit code. The application logic lives in
[`akiba-core`](../akiba-core).
