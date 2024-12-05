- commands
  - rename
    - animeRenamer.identify
      - anidb-udp client (with custom cache on ed2khash in ~/.cache/akiba/anidb/<hash>.fid.toml) fetch data
        - use https://github.com/tsukeero/anidb-udp-client (https://wiki.anidb.net/UDP_API_Definition) for file_by_hash call (we need a login it seems)
      - return AnimeRenamerEpisode { path: string; ed2khash: Ed2khash; data: AnimeFormatStringData; }
    - animeRenamer.rename(AnimeRenamerEpisode)
      - ensure format cannot contain /
      - allow targetPath to contain / and apply same format logic
      - allow target set via cli flag
    - mark as owned
  - batch (for cron where we run a rename + write-nfo on a directory)

- issues
  - jellyfin 10.10.x no longer pulls season from nfo when 0 ?
    -> tracked back to EpisodeNfoProvider, seems OK but unable to see what calls/inits this
```
sjorge@utena:/mm/anime/J/Jaku Chara Tomozaki-kun$ cat "Jaku Chara Tomozaki-kun - S2 - Most Strange Spell Names Have Unknown Origins (917D4650).nfo"
<?xml version="1.0" encoding="utf-8"?>
<episodedetails>
  <title>Most Strange Spell Names Have Unknown Origins</title>
  <uniqueid type="anidb" default="true">240767</uniqueid>
  <originaltitle>変な呪文の名前にはだいたい知られざる由来がある</originaltitle>
  <premiered>2021-06-02</premiered>
  <season>0</season>
  <episode>202</episode>
</episodedetails>
```
