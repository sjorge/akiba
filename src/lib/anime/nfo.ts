/*
 * Helper types and functions for writing tvshow.nfo files for Anime.
 *
 * This is not a full tvshow.nfo implementation and we omit a lot of
 *   optional fields.
 */
import type { AniDB_Show } from "anidbjs";

import braces from "braces";

import type { AnimeId, AnimeTitleVariant } from "lib/anime";
import type { EpisodeFile } from "lib/anime/resolver";
import type { TvShow, Episode, UniqueId } from "lib/nfo";

import { TvShowNfo, EpisodeNfo } from "lib/nfo";

export class AnimeShowNfo extends TvShowNfo {
  public constructor(id: AnimeId, metadata: AniDB_Show, path: string) {
    super(
      {
        uniqueId: [],
        title: metadata.titles[0].title.replace("`", "'"),
      } as TvShow,
      path,
    );

    this.updateMetadata(id, metadata);
  }

  private updateMetadata(id: AnimeId, metadata: AniDB_Show): void {
    // update uniqueids
    // WARN: avoid tvdb, it causes issues with jellyfin
    this.tvShow.uniqueId = [
      { type: "anidb", id: id.anidb, default: true } as UniqueId,
    ];
    if (id.anilist)
      this.tvShow.uniqueId.push({
        type: "anilist",
        id: id.anilist,
      } as UniqueId);
    if (id.tmdb)
      this.tvShow.uniqueId.push({
        type: "tmdb",
        id: id.tmdb,
      } as UniqueId);

    // update title (we prefer x-jat)
    this.tvShow.title = metadata.titles[0].title.replace("`", "'");
    for (const t of metadata.titles as AnimeTitleVariant[]) {
      if (t.type == "main" && t.language == "x-jat") {
        this.tvShow.title = t.title.replace("`", "'");
      } else if (t.type == "official" && t.language == "ja") {
        this.tvShow.originaltitle = t.title.replace("`", "'");
      }
    }

    // update premiered
    if (metadata.startDate && metadata.startDate.length == 10) {
      this.tvShow.premiered = metadata.startDate;
    } else if (metadata.startDate && metadata.startDate.length == 7) {
      this.tvShow.premiered = `${metadata.startDate}-01`;
    }

    // update poster
    if (metadata.picture)
      this.tvShow.poster = `https://cdn.anidb.net/images/main/${metadata.picture}`;

    // update rating (using jpn rating system)
    this.tvShow.mpaa = metadata.ageRestricted ? "18+" : "G";
  }
}

export class AnimeEpisodeNfo extends EpisodeNfo {
  public constructor(
    id: AnimeId,
    episodeFile: EpisodeFile,
    metadata: AniDB_Show,
  ) {
    super([], episodeFile.path);

    this.updateMetadata(id, episodeFile, metadata);
  }

  private updateMetadata(
    id: AnimeId,
    episodeFile: EpisodeFile,
    metadata: AniDB_Show,
  ): void {
    /*
     * A file can contain multiple episodes
     *
     * We can figure out range of episodes by using episode.episodeStart and episode.episodeEnd,
     *  luckily episode NFOs already assume that a file can have multiple episodes so we just need
     *  to collect all the episode metadata.
     */
    this.episode = [];

    const episodes: string[] =
      episodeFile.episodeStart == episodeFile.episodeEnd
        ? [episodeFile.episodeStart]
        : braces.expand(
            `{${episodeFile.episodeStart}..${episodeFile.episodeEnd}}`,
          );

    for (const episodeNumber of episodes) {
      for (const episodeMetadata of metadata.episodes) {
        // skip if episodeMetadata is for different episode
        if (episodeMetadata.episodeNumber != `${episodeNumber}`) continue;

        // base episode data
        const episode: Episode = {
          uniqueId: [
            {
              type: "anidb",
              id: episodeMetadata.id,
              default: true,
            } as UniqueId,
          ],
          title: episodeFile.title.replace("`", "'"),
        };

        // update episode title
        for (const t of episodeMetadata.titles as AnimeTitleVariant[]) {
          if (t.language == "en") {
            episode.title = t.title.replace("`", "'");
          } else if (t.language == "ja") {
            episode.originaltitle = t.title.replace("`", "'");
          }
        }

        // update season / episode
        switch (episodeMetadata.type) {
          case 1:
            episode.season = 1;
            episode.episode = parseInt(episodeMetadata.episodeNumber);
            break;
          default:
            episode.season = 0;
            episode.episode =
              parseInt(episodeMetadata.episodeNumber.substring(1)) +
              episodeMetadata.type * 100;
            break;
        }

        // update premiered
        if (episodeMetadata.airDate && episodeMetadata.airDate.length == 10) {
          episode.premiered = episodeMetadata.airDate;
        } else if (
          episodeMetadata.airDate &&
          episodeMetadata.airDate.length == 7
        ) {
          episode.premiered = `${episodeMetadata.airDate}-01`;
        }

        this.episode.push(episode);
      }
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
