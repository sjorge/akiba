# Akiba

Akiba is an utility for managing anime for Jellyfin, It can rename and export stub-NFO files.

Jellyfin does not like **AniDB**s absolute file numbering akiba can parse these and will query anidb to generate small stub NFO files.
Additionally akiba can identify and rename files similarly to `adbren`.

For best results enable only the **AniDB** provider in the library in Jellyfin as this is these are the only IDs we can guarantee to be present.
An attempt will be made to also map **Anilist** IDs, these are generally also present. Additionally **tmdb** tv IDs are also mapped but only for the first season.

## Known Issues
- [#13197](https://github.com/jellyfin/jellyfin/issues/13197) Jellyfin 10.10.x does not seem to parse episode NFOs season
- [#11575](https://github.com/jellyfin/jellyfin/issues/11575) Jellyfin 10.9.x+ needs to have `Display specials within seasons they aired in` checked in order to work around a bug with virtual seasons.

## Install

Install dependancies and compile static binary

> [!WARNING]
> The **postinstall** hook will compile a static binary, in addition to using `sed` to work around an issue in anidb-udp-client dependancy.
> I'm not proud of this hack, but until there is a new release, this is the only way to consistantly fix it.

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
