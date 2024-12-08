/*
 * Internal helper types and function for anidb
 */
import fs from "node:fs";
import path from "node:path";
import toml from "@iarna/toml";
import { machineIdSync } from "node-machine-id";
import { AnidbUDPClient } from "anidb-udp-client";

import type { AnidbCacheImplType } from "anidb-udp-client";

import { generateEd2kHash } from "lib/ed2khash";

import type { Config } from "lib/config";
import type { Ed2kHash } from "lib/ed2khash";
import type { AnimeStringFormatData } from "lib/anime/formatter";

type HashCache = {
  [path: string]: Ed2kHash;
};

// use a dummy cache as we will be caching a layer above AnidbUDPClient
class AnidbCacheDummy implements AnidbCacheImplType {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  public get<T>(key: string): Promise<T | undefined | null> {
    return Promise.resolve(null);
  }

  public set(
    key: string, // eslint-disable-line @typescript-eslint/no-unused-vars
    value: // eslint-disable-line @typescript-eslint/no-unused-vars
    string | number | Buffer | null | undefined | Record<string, unknown>,
  ): Promise<void> {
    return Promise.resolve();
  }
}

export type AnimeRenamerEpisode = {
  path: string;
  ed2khash: Ed2kHash;
  data?: AnimeStringFormatData;
};

export class AnimeRenamer {
  private anidb: AnidbUDPClient;
  private anidbUsername: string;
  private anidbPassword: string;
  private anidbApiKey: string | undefined;
  private anidbConnected: boolean = false;
  private format: string;
  private target: string;
  private rehash: boolean;
  private hashCacheAge: number;
  private hashCacheFile: string;
  private hashCache: HashCache;

  public constructor(
    config: Config,
    rehash: boolean = false,
    format?: string,
    target?: string,
  ) {
    if (
      config.anidb.udp_client.name === undefined ||
      config.anidb.udp_client.username === undefined ||
      config.anidb.udp_client.password === undefined
    )
      throw new Error(
        "AnimeRenamer requires at least config.anidb.upd_client to have name, username, and password set.",
      );

    // XXX: SimplePersistentCache always logs
    this.anidb = new AnidbUDPClient(config.anidb.udp_client.name, {
      cache: new AnidbCacheDummy(),
    });
    this.anidbUsername = config.anidb.udp_client.username;
    this.anidbPassword = config.anidb.udp_client.password;
    this.anidbApiKey = config.anidb.udp_client.api_key;

    this.format = format ? format : config.renamer.format;
    this.target = target
      ? target
      : config.renamer.target_path || path.resolve(".");

    this.hashCacheAge = config.cache.hash_age;
    this.hashCacheFile = path.join(
      config.cache.path,
      `${machineIdSync()}.hashes.toml`,
    );
    this.rehash = rehash;

    this.hashCache = this.readCache();
  }

  public toString(): string {
    return "AnimeRenamer";
  }

  private readCache(): HashCache {
    if (fs.existsSync(this.hashCacheFile)) {
      let cachePurged = false;
      const cache = toml.parse(
        fs.readFileSync(this.hashCacheFile, "utf8"),
      ) as HashCache;

      for (const episodeFile of Object.keys(cache)) {
        if (!fs.existsSync(episodeFile)) {
          // purge file that no longer exist
          delete cache[episodeFile];
          cachePurged = true;
        } else if (
          (Date.now() - cache[episodeFile]?.createdAt) / 1000 / 3600 / 24 >
          this.hashCacheAge
        ) {
          // purge file that are out of cache range
          delete cache[episodeFile];
          cachePurged = true;
        }
      }

      // update cache if needed
      if (cachePurged) this.writeCache(cache);

      return cache;
    }

    return {} as HashCache;
  }

  private writeCache(cache?: HashCache): void {
    // ensure cache dir exists
    fs.mkdirSync(path.dirname(this.hashCacheFile), {
      recursive: true,
      mode: 0o750,
    });

    // write cache file
    fs.writeFileSync(
      this.hashCacheFile,
      toml.stringify(cache ? cache : this.hashCache),
      {
        encoding: "utf8",
        mode: 0o660,
      },
    );
  }

  private async disconnect(): Promise<void> {
    if (!this.anidbConnected) return;

    await this.anidb.disconnect();

    this.anidbConnected = false;
  }

  private async connect(): Promise<void> {
    if (this.anidbConnected) return;

    await this.anidb.connect(
      this.anidbUsername,
      this.anidbPassword,
      this.anidbApiKey,
    );

    this.anidbConnected = true;
  }

  private async getAniDBFileByHash(
    file: Ed2kHash,
  ): Promise<AnimeStringFormatData | undefined> {
    await this.connect();

    // https://tsukeero.github.io/anidb-udp-client/stable/classes/AnidbUDPClient.html#constructor
    // XXX: set fields correctly
    const data = await this.anidb.file_by_hash(file.hash, file.size, [
      "aid",
      "gid",
      "eid",
      "romanji_name",
      "kanji_name",
      "english_name",
      "other_name",
      "anidb_filename",
      "epno",
      "ep_name",
      "ep_romanji_name",
      "ep_kanji_name",
      "group_name",
      "crc32",
      "type",
      "file_extension",
    ]);

    // XXX: process data into AnimeFormatStringData
    console.log(data);

    return undefined;
  }

  public async destroy(): Promise<void> {
    await this.disconnect();
  }

  public async identify(
    animeEpisodeFile: string,
    hashOnly: boolean = false,
  ): Promise<AnimeRenamerEpisode> {
    // calculate if rehash or cache is not fresh
    if (this.rehash || this.hashCache[animeEpisodeFile] === undefined) {
      // calculate hash
      this.hashCache[animeEpisodeFile] =
        await generateEd2kHash(animeEpisodeFile);

      // write cache file
      this.writeCache();
    }

    // quick return if hashing only
    if (hashOnly)
      return {
        path: animeEpisodeFile,
        ed2khash: this.hashCache[animeEpisodeFile],
      } as AnimeRenamerEpisode;

    // read episodeData from cache
    // XXX: Add own caching layer -> <hash>.fid.toml*
    let episodeData: AnimeStringFormatData | undefined;

    // read episodeData from AniDB if misisng
    if (episodeData === undefined) {
      episodeData = await this.getAniDBFileByHash(
        this.hashCache[animeEpisodeFile],
      );

      // write episodeData to cache
      // XXX: implement cache
    }

    return {
      path: animeEpisodeFile,
      ed2khash: this.hashCache[animeEpisodeFile],
      data: episodeData,
    } as AnimeRenamerEpisode;
  }

  public async rename(
    episode: AnimeRenamerEpisode,
    overwrite: boolean = false,
    symlink: boolean = true,
  ): Promise<void> {
    console.log(
      `TODO ${symlink ? "symlink" : "move"} ${episode.path} using metadata ${episode.data}, overwrite = ${overwrite}`,
    );
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
