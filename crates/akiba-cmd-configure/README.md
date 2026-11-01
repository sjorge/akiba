# akiba-cmd-configure

The `configure` command of [`akiba`](../akiba): update the configuration file
and optionally dump it as JSON.

The crate exposes `Args`, the `clap::Args` struct with the flags (including the
`--no-*` negations), and `run`, which returns the process exit code. The
application logic lives in [`akiba-core`](../akiba-core).
