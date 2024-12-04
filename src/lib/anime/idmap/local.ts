/*
 * Internal helpers for dealing with the local anime ID mapping
 */
import fs from "node:fs";
import path from "node:path";
import toml from "@iarna/toml";
import { deepmergeInto } from "deepmerge-ts";

import type { Config } from "lib/config";
import type { AnimeId } from "lib/anime";

export type LocalMapping = {
  [anidb: number]: {
    anilist?: number;
    tmdb?: number;
    tmdbSeason?: number;
  };
};

export class AnimeIdmapLocal {
  private mappingFile: string;
  private mappingCache: LocalMapping = {};

  public constructor(config: Config) {
    this.mappingFile = path.join(config.cache.path, "local.map.toml");
  }

  public toString(): string {
    return "AnimeLocalMapper";
  }

  public async refresh(): Promise<boolean> {
    try {
      if (fs.existsSync(this.mappingFile)) {
        deepmergeInto(
          this.mappingCache,
          toml.parse(fs.readFileSync(this.mappingFile, "utf8")) as LocalMapping,
        );

        return true;
      }

      return false;
    } catch {
      return false;
    }
  }

  public apply(id: AnimeId, overwrite: boolean = false): void {
    const aid = id.anidb;
    if (this.mappingCache[aid]?.anilist) {
      if (id.anilist === undefined || overwrite) {
        id.anilist = this.mappingCache[aid]?.anilist;
      }
    }

    if (this.mappingCache[aid]?.tmdb) {
      if (id.tmdb === undefined || overwrite) {
        id.tmdb = this.mappingCache[aid]?.tmdb;
        id.tmdbSeason = this.mappingCache[aid]?.tmdbSeason;
      }
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
