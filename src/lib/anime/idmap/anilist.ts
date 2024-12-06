/*
 * Internal helpers for dealing with anilist api
 */
import AniList from "anilist-node";
import levenshtein from "fast-levenshtein";

import type { Config } from "lib/config";
import type { AnimeId, AnimeTitleVariant } from "lib/anime";

import { AnimeResolver } from "lib/anime/resolver";

export class AnimeIdmapAnilistException extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AnimeIdmapAnilistException";
  }
}

export class AnimeIdmapAnilist {
  private anilist: AniList;
  private resolver: AnimeResolver;
  private fuzzyMatchThreshholdEng: number = 12; // account for 'Xth season' variants
  private fuzzyMatchThreshholdJpn: number = 5;

  public constructor(config: Config, resolver?: AnimeResolver) {
    if (config.anilist.token === undefined)
      throw new AnimeIdmapAnilistException("No Tmdb API key configured.");

    this.anilist = new AniList(config.anilist.token);
    this.resolver =
      resolver === undefined ? new AnimeResolver(config) : resolver;
  }

  public toString(): string {
    return "AnimeIdmapAnilist";
  }

  public async lookup(
    title: string,
    id: AnimeId,
    overwrite: boolean = false,
  ): Promise<void> {
    // quick return if we already have an anilist ID and are not overwriting
    if (id.anilist !== undefined && overwrite == false) return;

    // select official japanese title if available
    const mainTitle: AnimeTitleVariant[] = this.resolver
      .title(id)
      .filter((t: AnimeTitleVariant) => {
        if (t.type == "official" && t.language == "ja") {
          return t;
        } else if (t.type == "main" && t.language == "x-jat") {
          return t;
        }
      });

    // extract year variants
    for (const tv of mainTitle) {
      const titleYearRegEx = new RegExp(/^(.+)\s\((\d{4})\)$/);
      const t = tv as AnimeTitleVariant;

      if (t.year !== undefined) continue;

      const tilteYearMatch: RegExpExecArray | null = titleYearRegEx.exec(
        t.title,
      );
      if (tilteYearMatch !== null) {
        const nt: AnimeTitleVariant = {
          title: tilteYearMatch[1],
          year: parseInt(tilteYearMatch[2]),
          type: t.type,
          language: t.language,
        };
        mainTitle.push(nt);
      }
    }

    let exact_match: number | undefined;
    let best_match: number | undefined;
    let best_match_score: number = 0;
    for (const tv of mainTitle) {
      const t = tv as AnimeTitleVariant;

      if (exact_match == undefined) {
        const result = await this.anilist.searchEntry.anime(t.title);
        if (result?.media) {
          for (const media of result.media) {
            const mt =
              t.language == "ja" ? media.title.native : media.title.romaji;
            if (mt !== undefined && mt !== null) {
              if (t.year !== undefined) {
                // WARN: titles with an extract year should come first so we don't match
                //       the first season with the exact_match filter!
                const mediaData = await this.anilist.media.anime(media.id);
                if (mediaData?.seasonYear == t.year) {
                  const distance: number = levenshtein.get(
                    t.title.toLowerCase(),
                    mt.toLowerCase(),
                    { useCollator: true },
                  );

                  if (
                    distance <=
                    (t.language == "ja"
                      ? this.fuzzyMatchThreshholdJpn
                      : this.fuzzyMatchThreshholdEng)
                  ) {
                    if (
                      best_match == undefined ||
                      best_match_score > distance
                    ) {
                      best_match = media.id;
                      best_match_score = distance;
                    }
                  }
                }
              } else if (mt.toLowerCase() == t.title.toLowerCase()) {
                exact_match = media.id;
              } else {
                const distance: number = levenshtein.get(
                  t.title.toLowerCase(),
                  mt.toLowerCase(),
                  { useCollator: true },
                );
                if (distance == 0) {
                  exact_match = media.id;
                } else if (
                  distance <=
                  (t.language == "ja"
                    ? this.fuzzyMatchThreshholdJpn
                    : this.fuzzyMatchThreshholdEng)
                ) {
                  if (best_match == undefined || best_match_score > distance) {
                    best_match = media.id;
                    best_match_score = distance;
                  }
                }
              }
            }
          }
        }
      }
    }

    if (exact_match !== undefined) {
      id.anilist = exact_match;
    } else if (best_match !== undefined) {
      id.anilist = best_match;
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
