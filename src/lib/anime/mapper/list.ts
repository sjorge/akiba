/*
 * Internal helpers for dealing with the anime-list ID mapping
 */
import fs from "node:fs";
import path from "node:path";
import toml from "@iarna/toml";
import axios from "axios";
import { deepmergeInto } from "deepmerge-ts";

import type { Config } from "lib/config";
import type { AnimeId } from "lib/anime";

// NOTE: using fribb's anime-list (https://github.com/Fribb/anime-lists)
//       mini variant has a type to help with tmdb_ids, sadly we don't know the season offset.
const ListMapperURL =
  "https://raw.githubusercontent.com/Fribb/anime-lists/refs/heads/master/anime-list-mini.json";

type MiniListEntry = {
  livechart_id: number;
  thetvdb_id: number;
  "anime-planet_id": string;
  imdb_id: string;
  anisearch_id: number;
  themoviedb_id: number;
  anidb_id: number;
  kitsu_id: number;
  mal_id: number;
  type: "TV" | "MOVIE" | "SPECIAL" | "OVA" | "ONA" | "UNKNOWN";
  "notify.moe_id": string;
  anilist_id: number;
};

// NOTE: we only map to anilist, we are missing the tmdbSeason even if we filter on type=TV
type ListMapping = {
  [anidb: number]: {
    anilist?: number;
  };
};

export class AnimeListMapper {
  private mappingFile: string;
  private mappingCacheAge: number;
  private mappingCache: ListMapping = {};

  public constructor(config: Config) {
    this.mappingCacheAge = config.cache.mapping_age;
    this.mappingFile = path.join(config.cache.path, "list.map.toml");
  }

  public toString(): string {
    return "AnimeListMapper";
  }

  public async refresh(): Promise<boolean> {
    // load from cache if fresh
    if (fs.existsSync(this.mappingFile)) {
      const cacheStats = fs.statSync(this.mappingFile);
      if (
        (new Date().getTime() - cacheStats.mtimeMs) / 1000 / 3600 / 24 <
        this.mappingCacheAge
      ) {
        try {
          deepmergeInto(
            this.mappingCache,
            toml.parse(
              fs.readFileSync(this.mappingFile, "utf8"),
            ) as ListMapping,
          );
          return true;
        } catch {
          return false;
        }
      }
    }

    // load from remote
    try {
      this.mappingCache = {};

      const list = await axios.get(ListMapperURL);
      for (const entry of list.data as MiniListEntry[]) {
        const aid = entry.anidb_id;
        if (aid === undefined) continue;

        if (entry.anilist_id) {
          if (this.mappingCache[aid] === undefined) this.mappingCache[aid] = {};
          this.mappingCache[aid].anilist = entry.anilist_id;
        }
      }
    } catch {
      return false;
    }

    // write cache
    try {
      // ensure cache dir exists
      fs.mkdirSync(path.dirname(this.mappingFile), {
        recursive: true,
        mode: 0o750,
      });

      // write cache
      fs.writeFileSync(this.mappingFile, toml.stringify(this.mappingCache), {
        encoding: "utf8",
      });
      fs.chmodSync(this.mappingFile, 0o660);
    } catch {
      return false;
    }

    return true;
  }

  public apply(id: AnimeId, overwrite: boolean = false): void {
    const aid = id.anidb;

    if (this.mappingCache[aid]?.anilist) {
      if (id.anilist === undefined || overwrite) {
        id.anilist = this.mappingCache[aid]?.anilist;
      }
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
