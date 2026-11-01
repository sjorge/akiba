# akiba-cmd-rename

The `rename` command of [`akiba`](../akiba): identify episode files by their
ed2k hash through the AniDB UDP API and rename, copy or symlink them to a path
built from the template.

The crate exposes `Args`, the `clap::Args` struct with the flags, and `run`,
which returns the process exit code. The application logic lives in
[`akiba-core`](../akiba-core).
