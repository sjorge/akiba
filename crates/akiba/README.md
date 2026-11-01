# akiba

The `akiba` command line tool. It renames anime episode files, writes Jellyfin
NFO files and keeps local id mappings. The application logic lives in
[`akiba-core`](../akiba-core).

- `configure`: update the configuration file.
- `write-nfo`: write `tvshow.nfo` and episode NFO files, using the AniDB HTTP API.
- `rename`: identify episode files by their ed2k hash and rename them, using the AniDB UDP API.
- `idmap`: manage the local id mapping entries.

For installation and usage see the [top-level README](../../README.md).
