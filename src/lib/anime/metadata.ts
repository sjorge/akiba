/*
 * Internal helper types and function for anidb
 */
import { _DEFINE_PROG, _DEFINE_VER } from "vars";
import fs from "node:fs";
import path from "node:path";
import toml from "@iarna/toml";
import AniDB, { httpError } from "anidbjs";

import type { AniDB_Show } from "anidbjs";
import type { Config } from "lib/config";
import type { AnimeId } from "lib/anime";

export class AnimeMetadata {
  private anidb: AniDB;
  private cachePath: string;
  private cacheAge: number;

  public constructor(config: Config) {
    this.anidb = new AniDB(
      {
        client: config.anidb.client.name,
        version: config.anidb.client.version,
      },
      {
        headers: {
          "User-Agent": `${_DEFINE_PROG}/${_DEFINE_VER}`,
        },
      },
    );

    this.cachePath = config.cache.path;
    this.cacheAge = config.cache.metadata_age;
  }

  public toString(): string {
    return "AnimeMetadata";
  }

  public async get(id: AnimeId, refresh: boolean = false): Promise<AniDB_Show> {
    // load from cache if fresh
    const showCacheFile: string = path.join(
      this.cachePath,
      "anidb",
      `${id.anidb}.toml`,
    );

    if (fs.existsSync(showCacheFile) && !refresh) {
      const cacheStats = fs.statSync(showCacheFile);
      if (
        (new Date().getTime() - cacheStats.mtimeMs) / 1000 / 3600 / 24 <
        this.cacheAge
      ) {
        return toml.parse(fs.readFileSync(showCacheFile, "utf8")) as AniDB_Show;
      }
    }

    // try load from remote
    try {
      const metadata: AniDB_Show = await this.anidb.anime(id.anidb);

      // ensure cache dir exists
      fs.mkdirSync(path.dirname(showCacheFile), {
        recursive: true,
        mode: 0o750,
      });

      // write cache file
      fs.writeFileSync(showCacheFile, toml.stringify(metadata), {
        encoding: "utf8",
        mode: 0o660,
      });

      return metadata;
    } catch (_e: unknown) {
      const e = _e as httpError;

      switch (parseInt(e.code)) {
        case 302: // Client version missing or invalid
          throw new Error(
            "Please register a HTTP client on anidb and run 'configure' command again!",
          );
          break;
        case 500: // Banned
          throw new Error("Please try again in 24h, we are currently banned!");
          break;
        case 998: // Anime ID missing or invalid
          throw new Error(`Anime with ID ${id.anidb} is missing or invalid!`);
          break;
        default:
          throw new Error(`Could not retrieve metadata! ${e}`);
          break;
      }
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
