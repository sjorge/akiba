# akiba-core

The application library behind the [`akiba`](../akiba) command line tool. It is
not meant for use outside this workspace. For outside use see the
self-contained crates [`ed2k`](../ed2k), [`anidb-udp`](../anidb-udp) and
[`anidb-http`](../anidb-http), which do not depend on it.

It contains:

- `args`: the argument parsers shared by the commands (plain `&str` to
  `Result<T, String>` functions, no `clap`).
- `config`, `paths`, `host_id`: the `config.toml` model, the config and cache
  locations (`XDG_CONFIG_HOME` and `XDG_CACHE_HOME`, falling back to `~/.config`
  and `~/.cache`), and the host identity used for the cache file names.
- `cache`, `fs`, `fileinfo`: TOML cache files with age checks, and atomic file
  writes.
- `formatter`: the rename template formatter.
- `resolver`: the titles cache, anime id lookup from a name, and episode
  discovery in a show directory.
- `renamer`: identifies episode files by ed2k hash, looks them up on AniDB, and
  moves, copies or links them to a path built from the template.
- `nfo`: writes `tvshow.nfo` and episode NFO files.
- `idmap`: id mapping providers: the local mapping file, the anime-list mapping,
  and the AniList and TMDB title searches.
- `logger`: user facing output.
- `anime`, `http`, `text`: shared id and title types, a blocking HTTP trait so
  tests can use a fake, and the text helpers.
