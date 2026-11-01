# Akiba

Akiba is a utility for managing anime for Jellyfin. It can rename and export stub-NFO files.

Jellyfin does not like **AniDB**'s absolute file numbering. Akiba can parse these and will query AniDB to generate small stub NFO files.
Additionally akiba can identify and rename files similarly to `adbren`.

For best results enable only the **AniDB** provider in the library in Jellyfin, as these are the only IDs we can guarantee to be present.
An attempt will be made to also map **Anilist** IDs, these are generally also present. Additionally **tmdb** tv IDs are also mapped but only for the first season.

## Known Issues
- [#11575](https://github.com/jellyfin/jellyfin/issues/11575) Jellyfin 10.9.x+ needs to have `Display specials within seasons they aired in` checked in order to work around a bug with virtual seasons.
- [#13197](https://github.com/jellyfin/jellyfin/issues/13197) ~~Jellyfin 10.10.x does not seem to parse episode NFOs season~~ Fixed in 10.10.7

## Install

Akiba 3.0.0 is built from source with Cargo. There are no release binaries.

Supported platforms: Linux and macOS, plus illumos, where building has been confirmed. FreeBSD is a goal as well, but a build there has not been confirmed yet.

### Building from source

1. Install the Rust toolchain. `rust-toolchain.toml` pins Rust 1.99.1, and rustup installs that version automatically inside the checkout. Use rustup if your distribution's `cargo` is older than 1.99.1.
2. Install the system TLS library. Akiba uses the platform's native TLS, which is OpenSSL on Linux, FreeBSD and illumos:
   - Debian and Ubuntu: `sudo apt install build-essential pkg-config libssl-dev`
   - Fedora: `sudo dnf install gcc pkgconf-pkg-config openssl-devel`
   - Arch Linux: `sudo pacman -S base-devel openssl pkgconf`
   - FreeBSD: `pkg install rust openssl pkgconf`
   - illumos: install the OpenSSL development package of your distribution (headers and `openssl.pc`)
   - macOS: nothing extra, beyond the Xcode command line tools that provide the C linker (`xcode-select --install`)

   If OpenSSL is installed where `pkg-config` cannot find it, set `OPENSSL_DIR` to its prefix before building.
3. From a clone of this repository, build the release binary:

   ```bash
   cargo build --release
   ```

   The binary is `target/release/akiba`. Copy it somewhere in your `PATH`, or install it with Cargo (below).

To install into Cargo's bin directory (`~/.cargo/bin` by default):

```bash
cargo install --locked --path crates/akiba
```

`--locked` makes Cargo use the exact dependency versions in `Cargo.lock`.

Check the result with `akiba --version`, which prints `3.0.0`.

### Custom certificate trust

By default akiba trusts the system certificate store, which it finds automatically. If your network uses a custom CA, point akiba at a PEM bundle. Only the certificates in that file are trusted then:

```bash
akiba configure --tls-ca-file /path/to/ca-bundle.pem
# back to the system certificate store
akiba configure --no-tls-ca-file
```

This writes the `[tls]` table `ca_file` in the configuration file.

### Files

- Configuration: `config.toml` in `$XDG_CONFIG_HOME/akiba` (default `~/.config/akiba`), or the file named by `AKIBA_CONFIG`.
- Cache: the `akiba` directory under `$XDG_CACHE_HOME` (default `~/.cache/akiba`).

The configuration and cache are compatible with akiba 2.x: an existing `config.toml` and cache are used as they are, and rename templates render as before.

## Usage

*Some configuration is required for akiba to be useable. The minimal configuration changes depending on the command you wish to use.*

The cache path can only be changed at first run.
To use a non-default cache path set the `XDG_CACHE_HOME` variable before running the **configure** command the first time.

```bash
export XDG_CACHE_HOME="/mnt/anime/.cache"
akiba configure
```

### Command: idmap

To manage local **aid** to **anilist** id or **tmdb** tv id mappings, you do not need any additional configuration to run this command.

```bash
# Map the tmdb tv series for Ranma1/2 (2024)
akiba idmap 18700 --tmdbid 259140

# Unmap the tmdb tv series for Ranma1/2 (2024)
akiba idmap 18700 --no-tmdbid --no-show

# Blacklist a potential incorrect mapping from the tmdb idmap provider
# (-1 is an alias for blacklist, --anilistid works the same)
akiba idmap 18700 --tmdbid blacklist

# Show additional flags
akiba idmap --help
```

### Command: write-nfo

Writes tvshow.nfo and episode.nfo files, this requires that an *AniDB HTTP Client* identifier is configured.

Please register a project on https://anidb.net/software/add, and add a HTTP client.

```bash
# required
akiba configure --anidb-http-client examplehttpclient/1
# optional: enable saving poster.jpg
akiba configure --anidb-poster
# optional: overwrite nfo files by default
akiba configure --overwrite-nfo
```

Aside from using the local `akiba idmap` command, we also use [Fribb's Anime-List](https://github.com/Fribb/anime-lists).
If a verified mapping is not present in either of these, the **Anilist** and **tmdb** API can be used to do a search. This however required additional configuration.

#### Requesting: *anilist token*
1. visit https://anilist.co/settings/developer
1. click *Create New Client*
1. enter `https://anilist.co/api/v2/oauth/pin` as the *Redirect URL*
1. approve the generated token by visiting `https://anilist.co/api/v2/oauth/authorize?client_id={clientID}&response_type=token` (do not forget to replace clientID in the URL!)

```bash
akiba configure --anilist-token MY_VERY_LONG_TOKEN_STRING_HERE
```

#### Requesting: *tmdb api key*

[TMDBs Getting Started](https://developer.themoviedb.org/docs/getting-started)

```bash
akiba configure --tmdb-api-key TMDB_API_KEY
```

### Command: rename

Tries to identify and rename files, this requires that an *AniDB UDP Client* identifier is configured.

Please register a project on https://anidb.net/software/add, and add a UDP client and your username and password is configured

```bash
# required: udp client and authentication
akiba configure --anidb-udp-client exampleudpclient/1
akiba configure --anidb-auth-username clevergirl --anidb-auth-password hunter1
# optional: set UDP API key to enable encryption
# It's labeled "UDP API Key" under your profile, but it's more of a PSK
akiba configure --anidb-auth-api-key swordfish
# optional: set the filename format
akiba configure --rename-format "{anime_name_romaji:upper_first}/{anime_name_romaji}/{anime_name_romaji} - {episode:number} - {episode_name} ({crc32:lower}).{filetype}"
# optional: set target directory (must be an existing directory; restore to use current work directory with --no-rename-target-path)
akiba configure --rename-target-path /mnt/anime
```

If you are migrating from `adbren` here are the presets updates for use with akiba.

```
"{anime_name_english}_{episode}{version}-{group_short}.{filetype}"
"{anime_name_english}_{episode}{version}_{episode_name}-{group_short}.{filetype}"
"{anime_name_english}_{episode}{version}_{episode_name}-{group_short}({crc32}).{filetype}"
"{anime_name_romaji} - {episode} - {episode_name} ({crc32}).{filetype}"
"[{group_short}] {anime_name_english} - {episode} - {episode_name} ({crc32}).{filetype}"
```

## Workspace

The repository is a Cargo workspace. Three crates are standalone libraries, with no dependency on akiba's own code, and can be used on their own:

- [`ed2k`](crates/ed2k/README.md): streaming ed2k hashing
- [`anidb-udp`](crates/anidb-udp/README.md): client for the AniDB UDP API
- [`anidb-http`](crates/anidb-http/README.md): client for the AniDB HTTP API and the titles dump

The `akiba` binary is built from `akiba-core` and the `akiba-cmd-*` command crates. Each crate has its own README.

## Tests

```bash
cargo test --workspace
```

This runs the unit tests and the fake-server tests. The UDP fake-server tests bind local UDP ports, so they need a host that allows that.

### Live tests (ignored by default)

The ignored tests in `crates/anidb-udp/tests/live.rs` talk to the real AniDB server and need your AniDB credentials.

> [!WARNING]
> AniDB bans an IP that sends requests too fast or from many source ports. A ban lasts about 30 minutes and applies to every client on that IP. Read [crates/anidb-udp/README.md](crates/anidb-udp/README.md) before running these tests, and never run them in a loop.

```bash
AKIBA_LIVE_USER=... AKIBA_LIVE_PASS=... AKIBA_LIVE_CLIENT=... \
  cargo test -p anidb-udp --test live -- --ignored --nocapture --test-threads=1
```

Set `AKIBA_LIVE_APIKEY` as well to run the encrypted tests too.
