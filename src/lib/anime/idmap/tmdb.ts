/*
 * Internal helpers for dealing with anilist api
 */
import { MovieDb } from "moviedb-promise";

import type { Config } from "lib/config";
import type { AnimeId, AnimeTitleVariant } from "lib/anime";

import { AnimeResolver } from "lib/anime/resolver";

export class AnimeIdmapTmdbException extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AnimeIdmapTmdbException";
  }
}

export class AnimeIdmapTmdb {
  private tmdb: MovieDb;
  private resolver: AnimeResolver;

  public constructor(config: Config, resolver?: AnimeResolver) {
    if (config.tmdb.api_key === undefined)
      throw new AnimeIdmapTmdbException("No Tmdb API key configured.");

    this.tmdb = new MovieDb(config.tmdb.api_key);
    this.resolver =
      resolver === undefined ? new AnimeResolver(config) : resolver;
  }

  public toString(): string {
    return "AnimeIdmapTmdb";
  }

  public async apply(id: AnimeId, overwrite: boolean = false): Promise<void> {
    // quick return if we already have a tmdb ID and are not overwriting
    if (id.tmdb !== undefined && overwrite == false) return;

    // select official japanese title if available
    const mainTitle: AnimeTitleVariant[] = this.resolver
      .title(id)
      .filter((t: AnimeTitleVariant) => {
        if (t.type == "official" && ["ja", "en"].includes(t.language)) {
          return t;
        } else if (t.type == "main" && t.language == "x-jat") {
          return t;
        }
      });

    let exact_match: number | undefined;
    let best_match: number | undefined;

    for (const tv of mainTitle) {
      const t = tv as AnimeTitleVariant;

      if (exact_match == undefined) {
        const tmdbData = await this.tmdb.searchTv({
          query: t.title,
          include_adult: true,
        });

        tmdbData?.results?.forEach((media) => {
          // ignore tv shows without genre_id 16 (Animation)
          if (media.genre_ids?.includes(16) && exact_match == undefined) {
            const mt =
              t.language == "ja" && media.original_language == "ja"
                ? media.original_name
                : media.name;
            if (mt !== undefined && mt !== null) {
              if (mt.toLowerCase() == t.title.toLowerCase()) {
                exact_match = media.id;
              } else if (
                t.language == "ja" &&
                media.original_language == "ja"
              ) {
                let tmdbNormalizedTitle = t.title.toLowerCase();
                tmdbNormalizedTitle = tmdbNormalizedTitle.replace("&", "and");
                tmdbNormalizedTitle = tmdbNormalizedTitle
                  .replace("[", "【")
                  .replace("]", "】");
                tmdbNormalizedTitle = tmdbNormalizedTitle.replace(/\.$/, "。");
                tmdbNormalizedTitle = tmdbNormalizedTitle.replace(",", "、");

                if (mt.toLowerCase() == tmdbNormalizedTitle)
                  if (best_match == undefined) {
                    best_match = media.id;
                  }
              }
            }
          }
        });
      }
    }

    if (exact_match !== undefined) {
      id.tmdb = exact_match;
      id.tmdbSeason = 1;
    } else if (best_match !== undefined) {
      id.tmdb = best_match;
      id.tmdbSeason = 1;
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
