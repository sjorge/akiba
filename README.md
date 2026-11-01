# Akiba

Akiba is an utility for managing anime for Jellyfin, It can rename and export stub-NFO files.

Jellyfin does not like **AniDB**s absolute file numbering akiba can parse these and will query anidb to generate small stub NFO files.
Additionally akiba can identify and rename files similarly to `adbren`.

For best results enable only the **AniDB** provider in the library in Jellyfin as this is these are the only IDs we can guarantee to be present.
An attempt will be made to also map **Anilist** IDs, these are generally also present. Additionally **tmdb** tv IDs are also mapped but only for the first season.

## Known Issues
- [#11575](https://github.com/jellyfin/jellyfin/issues/11575) Jellyfin 10.9.x+ needs to have `Display specials within seasons they aired in` checked in order to work around a bug with virtual seasons.
- [#13197](https://github.com/jellyfin/jellyfin/issues/13197) ~~Jellyfin 10.10.x does not seem to parse episode NFOs season~~ Fixed in 10.10.7

## Install

Install dependancies and compile static binary

> [!NOTE]
> The **postinstall** hook will compile a static binary.

```bash
bun install
```

Copy `bin/akiba` to somewhere in your path.

```bash
# local user
cp bin/akiba ~/.local/bin

# system wide
sudo cp bin/akiba /usr/local/bin
```

## Usage

*Some configuration is required for akiba to be useable. The minimal configuration changes depending on the command you wish to use.*

The cache path can only be changed at first run. (There is no code in place to handle cache path moves)
To use a non-default cache path set the `XDG_CACHE_HOME` variable before running the **configure** command the first time.

```bash
export XDG_CACHE_HOME="/mnt/anime/.cache"
akiba configure
```

### Command: idmap

To manage local **aid** to **anilist** id or **tmdb** tv id mappings, you do not need any additional configuration to run this command.o

```bash
# Map the tmdb tv series for Ranma1/2 (2024)
akiba idmap 18700 --tmdbid 259140

# Unmap the tmdb tv series for Ranma1/2 (2024)
akiba idmap 18700 --no-tmdbid --no-show

# Unmap the anilist entry for Ranma1/2 (2024)
akiba idmap 18700 --no-anilistid --no-show

# Blacklist the tmdb lookup for this anime, so it is not looked up again
# (`--tmdbid -1` is the same; unlike --no-tmdbid, which looks it up again)
akiba idmap 18700 --tmdbid blacklist

# Blacklist the anilist lookup for this anime, so it is not looked up again
# (`--anilistid -1` is the same; unlike --no-anilistid, which looks it up again)
akiba idmap 18700 --anilistid blacklist

# Show additional flags
akiba idmap --help
```

A blacklisted **anilist** or **tmdb** id is skipped by the automatic lookups, and it is left out of the NFO files. Use it when an anime has no entry on that site, or when the search keeps finding the wrong one.

### Command: write-nfo

Writes tvshow.nfo and episode.nfo files, this requires that an *AniDB HTTP Client* identifier is configured.

Please register a project on https://anidb.net/software/add, and add a HTTP client.

```bash
# required
akiba configure --anidb-http-client examplehttpclient/1
# optional: enable saving poster.jpg
akiba configure --anidb-poster
# optional: overwrite existing nfo and poster.jpg files by default
akiba configure --overwrite-nfo
```

Existing NFO files are kept unless they are overwritten. Use `--force` for a single run, or set `overwrite_nfo` with `akiba configure --overwrite-nfo` to overwrite them by default (`--no-overwrite-nfo` turns that off again). The config key is honoured without `--force`. Both also replace an existing `poster.jpg`, which is only downloaded when `--anidb-poster` is enabled.

```bash
# overwrite the existing NFO files and poster.jpg of one anime
akiba write-nfo /mnt/anime/Ranma --force
```

The NFO files get their modification time from AniDB: `tvshow.nfo` gets the start date of the anime, and each episode NFO gets the air date of the first episode in its file. A date with only a year, or a year and a month, counts as the first day of that period. The time is noon UTC, so the date reads the same in every time zone. If the date is missing or before 1970, the file keeps the time it was written with.

Aside from using the local `akiba idmap` command, we also use [Fribb's Anime-List](https://github.com/Fribb/anime-lists).
If a verified mapping is not present in either of these, the **Anilist** and **tmdb** API can be used to do a search. This however required additional configuration.

#### Requesting: *anilist token*
1. visit https://anilist.co/settings/developer
1. click *Create New Client*
1. enter `https://anilist.co/api/v2/oauth/pin` as the *Redirect URL*
1. approve the generated token by visting `https://anilist.co/api/v2/oauth/authorize?client_id={clientID}&response_type=token` (do not forget to replace clientID in the URL!)

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
# optional: set target directory (restore to use current work directory with --no-rename-target-path)
akiba configure --rename-target-path /mnt/anime
```

If you are migrating from `adbren` here are the presets updates for use with akiba.

```
"{anime_name_english}_{episode}{version]-{group_short}.{filetype}"
"{anime_name_english}_{episode}{version}_{episode_name}-{group_short}.{filetype}"
"{anime_name_english}_{episode}{version}_{episode_name}-{group_short}({crc32}).{filetype}"
"{anime_name_romaji} - {episode} - {episode_name} ({crc32}).{filetype}"
"[{group_short}] {anime_name_english} - {episode} - {episode_name} ({crc32}).{filetype}"
```

#### Sidecar files

Files that share the name of an episode follow it when `rename` moves, copies or symlinks the episode. This covers `.nfo` files and subtitles (`srt`, `ass`, `ssa`, `vtt`, `sub`, `idx`, `sup` and `mks`), with or without a language tag. `Show - 01.mkv` takes `Show - 01.nfo` and `Show - 01.en.srt` along, and they are renamed to match the new name. A file that belongs to a longer episode stays with that episode, so `Show - 1.5.srt` goes with `Show - 1.5.mkv` and not with `Show - 1.mkv`.

An existing file at the destination is kept, unless `--force` is given, which overwrites it the same way as the episode. With `--copy` the originals stay where they are, and with `--symlink` a symlink to the new file is left at each old path. A move across file systems copies the file and then removes the original.

`--copy` and a move across file systems keep the modification time of the original, for the episode and its sidecars alike.
