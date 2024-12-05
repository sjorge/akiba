/*
 * Helper types and functions for writing tvshow.nfo files for Anime.
 *
 * This is not a full tvshow.nfo implementation and we omit a lot of
 *   optional fields.
 */
import type { AniDB_Show } from "anidbjs";

import type { AnimeId, AnimeTitleVariant } from "lib/anime";
import type { TvShow, UniqueId } from "lib/nfo";

import { TvShowNfo } from "lib/nfo";

export class AnimeShowNfo extends TvShowNfo {
  public constructor(id: AnimeId, metadata: AniDB_Show, path: string) {
    super(
      {
        uniqueId: [{ type: "anidb", id: id.anidb, default: true } as UniqueId],
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
    this.tvShow.poster = metadata.picture;

    // update rating (using jpn rating system)
    this.tvShow.mpaa = metadata.ageRestricted ? "18+" : "G";
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
