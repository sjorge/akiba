/*
 * Internal helper types and function for anidb
 */
import fs from "node:fs";
import path from "node:path";
import toml from "@iarna/toml";
import Bun from "bun";
import { EventEmitter } from "events";
import { machineIdSync } from "node-machine-id";
import { AnidbUDPClient, AnidbError } from "anidb-udp-client";

import type { AnidbCacheImplType } from "anidb-udp-client";

import { generateEd2kHash } from "lib/ed2khash";

import type { Config } from "lib/config";
import type { Ed2kHash } from "lib/ed2khash";
import {
  animeStringFormat,
  type AnimeStringFormatData,
} from "lib/anime/formatter";

export type AniDBMylistState =
  | "internal_storage"
  | "external_storage"
  | "remote_storage"
  | "deleted"
  | "unknown";

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
  action?: "move" | "copy" | "symlink" | "uptodate";
  destination_path?: string;
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
  private cachePath: string;
  private cacheAgeMetadata: number;
  private cacheAgeHash: number;
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

    this.anidb = new AnidbUDPClient(config.anidb.udp_client.name, {
      cache: new AnidbCacheDummy(),
    });
    this.anidbUsername = config.anidb.udp_client.username;
    this.anidbPassword = config.anidb.udp_client.password;
    this.anidbApiKey = config.anidb.udp_client.api_key;

    this.cachePath = config.cache.path;
    this.cacheAgeMetadata = config.cache.mapping_age;
    this.cacheAgeHash = config.cache.hash_age;

    this.format = format ? format : config.renamer.format;
    this.target = target
      ? target
      : config.renamer.target_path || path.resolve(".");
    this.rehash = rehash;

    this.hashCache = this.readHashCache();

    // anidb-udp-client goes over the default 10 limit
    EventEmitter.defaultMaxListeners = 64;
    this.anidb.setMaxListeners(64);
  }

  public toString(): string {
    return "AnimeRenamer";
  }

  private readHashCache(): HashCache {
    const hashCacheFile = path.join(
      this.cachePath,
      `${machineIdSync()}.hashes.toml`,
    );
    if (fs.existsSync(hashCacheFile)) {
      let cachePurged = false;
      const cache = toml.parse(
        fs.readFileSync(hashCacheFile, "utf8"),
      ) as HashCache;

      for (const episodeFile of Object.keys(cache)) {
        if (!fs.existsSync(episodeFile)) {
          // purge file that no longer exist
          delete cache[episodeFile];
          cachePurged = true;
        } else if (
          (Date.now() - cache[episodeFile]?.createdAt) / 1000 / 3600 / 24 >
          this.cacheAgeHash
        ) {
          // purge file that are out of cache range
          delete cache[episodeFile];
          cachePurged = true;
        }
      }

      // update cache if needed
      if (cachePurged) this.writeHashCache(cache);

      return cache;
    }

    return {} as HashCache;
  }

  private writeHashCache(cache?: HashCache): void {
    const hashCacheFile = path.join(
      this.cachePath,
      `${machineIdSync()}.hashes.toml`,
    );

    // ensure cache dir exists
    fs.mkdirSync(this.cachePath, {
      recursive: true,
      mode: 0o750,
    });

    // write cache file
    fs.writeFileSync(
      hashCacheFile,
      toml.stringify(cache ? cache : this.hashCache),
      {
        encoding: "utf8",
        mode: 0o660,
      },
    );
  }

  private async readEpisodeDataCache(
    animeEpisodeHash: Ed2kHash,
    refresh: boolean = false,
  ): Promise<AnimeStringFormatData | undefined> {
    const episodeCacheFile: string = path.join(
      this.cachePath,
      "anidb",
      `${animeEpisodeHash.hash}.fid.toml`,
    );

    // read episodeData from cache
    if (fs.existsSync(episodeCacheFile) && !refresh) {
      const cacheStats = fs.statSync(episodeCacheFile);
      if (
        (new Date().getTime() - cacheStats.mtimeMs) / 1000 / 3600 / 24 <
        this.cacheAgeMetadata
      ) {
        return toml.parse(
          fs.readFileSync(episodeCacheFile, "utf8"),
        ) as AnimeStringFormatData;
      }
    }

    return undefined;
  }

  private async writeEpisodeDataCache(
    animeEpisodeHash: Ed2kHash,
    episodeData: AnimeStringFormatData | undefined,
  ): Promise<void> {
    const episodeCacheFile: string = path.join(
      this.cachePath,
      "anidb",
      `${animeEpisodeHash.hash}.fid.toml`,
    );

    // ensure cache dir exists
    fs.mkdirSync(path.dirname(episodeCacheFile), {
      recursive: true,
      mode: 0o750,
    });

    // write cache file
    if (episodeData !== undefined)
      fs.writeFileSync(episodeCacheFile, toml.stringify(episodeData), {
        encoding: "utf8",
        mode: 0o660,
      });
  }

  private async disconnect(): Promise<void> {
    if (!this.anidbConnected) return;

    await this.anidb.disconnect();

    this.anidbConnected = false;
  }

  private async connect(): Promise<void> {
    if (this.anidbConnected) return;

    try {
      await this.anidb.connect(
        this.anidbUsername,
        this.anidbPassword,
        this.anidbApiKey,
      );
      this.anidbConnected = true;
    } catch (_e: unknown) {
      const e = _e as AnidbError;
      await this.anidb.disconnect();
      throw e;
    }
  }

  private async getAniDBFileByHash(
    file: Ed2kHash,
  ): Promise<AnimeStringFormatData | undefined> {
    await this.connect();

    try {
      const data = await this.anidb.file_by_hash(file.hash, file.size, [
        "aid",
        "gid",
        "eid",
        "gid",
        "mylist_id",
        "state",
        "size",
        "ed2k",
        "md5",
        "sha1",
        "crc32",
        "dub_language",
        "sub_language",
        "quality",
        "source",
        "audio_codecs",
        "audio_bitrates",
        "video_codec",
        "video_bitrate",
        "video_resolution",
        "file_extension",
        "length_seconds",
        "description",
        "group_name",
        "group_short_name",
        "epno",
        "ep_name",
        "ep_romanji_name",
        "ep_kanji_name",
        "anime_total_episodes",
        "highest_episode",
        "year",
        "type",
        "romanji_name",
        "kanji_name",
        "english_name",
        "other_name",
        "short_name_list",
        "synonym_list",
        "category_list",
        "anidb_filename",
      ]);

      // status_code parsing
      //const STATUS_CRCOK = 0x01;
      //const STATUS_CRCERR = 0x02;
      const STATUS_ISV2 = 0x04;
      const STATUS_ISV3 = 0x08;
      const STATUS_ISV4 = 0x10;
      const STATUS_ISV5 = 0x20;
      const STATUS_UNC = 0x40;
      const STATUS_CEN = 0x80;

      const status_code = parseInt(`${data.state}`, 10);

      let version = "";
      if ((status_code & STATUS_ISV2) > 0) version = "v2";
      if ((status_code & STATUS_ISV3) > 0) version = "v3";
      if ((status_code & STATUS_ISV4) > 0) version = "v4";
      if ((status_code & STATUS_ISV5) > 0) version = "v5";

      let censored = "";
      if ((status_code & STATUS_CEN) > 0) censored = "cen";
      if ((status_code & STATUS_UNC) > 0) censored = "unc";

      return {
        fid: parseInt(`${data.fid}`, 10), // typedef as number but actually returns string
        aid: data.aid,
        eid: data.eid,
        gid: data.gid,
        lid: data.mylist_id,
        status: parseInt(`${data.state}`, 10),
        size: data.size,
        ed2k: data.ed2k,
        md5: data.md5,
        sha1: data.sha1,
        crc32: data.crc32,
        lang_dub: data.dub_language,
        lang_sub: data.sub_language,
        quaility: data.quality,
        source: data.source,
        audio_codec: data.audio_codecs,
        audio_bitrate: data.audio_bitrates,
        video_codec: data.video_codec,
        video_bitrate: data.video_bitrate,
        resolution: data.video_resolution,
        filetype: data.file_extension,
        length: data.length_seconds,
        description: data.description,
        group: data.group_name,
        group_short: data.group_short_name,
        episode: data.epno,
        episode_name: data.ep_name,
        episode_name_romaji: data.ep_romanji_name,
        episode_name_kanji: data.ep_kanji_name,
        episode_total: data.anime_total_episodes,
        episode_last: data.highest_episode,
        anime_year: data.year.split("-").map((year: string) => parseInt(year)), // NOTE: can be YYYY-YYYY
        anime_type: data.type,
        anime_name_romaji: data.romanji_name,
        anime_name_kanji: data.kanji_name,
        anime_name_english: data.english_name,
        anime_name_other: data.other_name.split("'"), // XXX: this is broken upstream https://github.com/tsukeero/anidb-udp-client/pull/21
        anime_name_short: data.short_name_list,
        anime_synonyms: data.synonym_list,
        anime_category: data.category_list,
        version: version,
        censored: censored,
        orginal_name: data.anidb_filename,
      } as AnimeStringFormatData;
    } catch (_e: unknown) {
      const e = _e as AnidbError;
      switch (e.code) {
        case 320: // NO_SUCH_FILE
          return undefined;
        default:
          await this.disconnect();
          throw _e as Error;
      }
    }
  }

  public async destroy(): Promise<void> {
    await this.disconnect();
  }

  public async identify(
    animeEpisodeFile: string,
    refresh: boolean = false,
    hashOnly: boolean = false,
  ): Promise<AnimeRenamerEpisode> {
    // sanity check parameters
    if (!fs.existsSync(animeEpisodeFile))
      throw new Error(`${animeEpisodeFile} does not exist!`);
    if (!fs.statSync(animeEpisodeFile).isFile())
      throw new Error(`${animeEpisodeFile} must be a file!`);

    // calculate if rehash or cache is not fresh
    if (
      this.rehash ||
      this.hashCache[animeEpisodeFile] === undefined ||
      this.hashCache[animeEpisodeFile].size !=
        fs.statSync(animeEpisodeFile).size
    ) {
      // calculate hash
      this.hashCache[animeEpisodeFile] =
        await generateEd2kHash(animeEpisodeFile);

      // write cache file
      this.writeHashCache();
    }

    // quick return if hashing only
    if (hashOnly)
      return {
        path: animeEpisodeFile,
        ed2khash: this.hashCache[animeEpisodeFile],
      } as AnimeRenamerEpisode;

    // read episodeData from cache
    let episodeData = await this.readEpisodeDataCache(
      this.hashCache[animeEpisodeFile],
      refresh,
    );

    // read episodeData from AniDB (if not cached)
    if (episodeData === undefined) {
      episodeData = await this.getAniDBFileByHash(
        this.hashCache[animeEpisodeFile],
      );

      if (
        episodeData?.crc32 == "" ||
        episodeData?.sha1 == "" ||
        episodeData?.md5 == ""
      ) {
        const data = fs.readFileSync(animeEpisodeFile, { encoding: "binary" });

        episodeData.crc32 = Bun.hash
          .crc32(Buffer.from(data, "binary"))
          .toString(16);

        episodeData.md5 = new Bun.CryptoHasher("md5")
          .update(data, "binary")
          .digest("hex");

        episodeData.sha1 = new Bun.CryptoHasher("sha1")
          .update(data, "binary")
          .digest("hex");
      }

      await this.writeEpisodeDataCache(
        this.hashCache[animeEpisodeFile],
        episodeData,
      );
    }

    // return renamer data
    return {
      path: animeEpisodeFile,
      ed2khash: this.hashCache[animeEpisodeFile],
      data: episodeData,
    } as AnimeRenamerEpisode;
  }

  public async rename(
    episode: AnimeRenamerEpisode,
    overwrite: boolean = false,
    copy: boolean = false,
    symlink: boolean = false,
    dryRun: boolean = false,
  ): Promise<AnimeRenamerEpisode> {
    // missing metadata
    if (episode.data === undefined) return episode;

    const sourcePath = episode.path;
    const destinationPath = path.join(
      this.target,
      animeStringFormat(this.format, episode.data),
    );

    // file already correct path
    if (sourcePath == destinationPath) {
      episode.destination_path = destinationPath;
      episode.action = "uptodate";
      return episode;
    }

    // abort if destinationPath exists and !overwrite
    if (fs.existsSync(destinationPath) && !overwrite) return episode;

    // create parent path
    if (!dryRun)
      fs.mkdirSync(path.dirname(destinationPath), {
        recursive: true,
      });

    // rename episode
    if (copy) {
      // NOTE: fs.copyFileSync overwrites by default
      if (!dryRun) fs.copyFileSync(sourcePath, destinationPath);

      episode.destination_path = destinationPath;
      episode.action = "copy";
    } else {
      try {
        if (!dryRun) fs.renameSync(sourcePath, destinationPath);
      } catch (_e: unknown) {
        const e = _e as Error;
        if (e.message != "Cross-device link") throw e;

        // fallback to copy for filesystem boundries
        fs.copyFileSync(sourcePath, destinationPath);
        fs.unlinkSync(sourcePath);
      }

      episode.destination_path = destinationPath;
      episode.action = "move";

      if (symlink) {
        if (!dryRun) fs.symlinkSync(destinationPath, sourcePath);
        episode.action = "symlink";
      }
    }

    // update hashes
    if (!dryRun) {
      // update link in ed2hash
      const ed2hash = this.hashCache[episode.path];
      ed2hash.link = `ed2k://|file|${path.basename(episode.destination_path)}|${ed2hash.size}|${ed2hash.hash}|/`;

      if (!copy) delete this.hashCache[episode.path];
      this.hashCache[episode.destination_path] = ed2hash;
      this.writeHashCache();
    }

    return episode;
  }

  public async setMylistState(
    episode: AnimeRenamerEpisode,
    mylistState: AniDBMylistState,
  ): Promise<boolean> {
    await this.connect();

    let needUpdate;
    let mylistEdit;

    try {
      const list_data = await this.anidb.mylist_by_file_hash(
        episode.ed2khash.hash,
        episode.ed2khash.size,
      );

      needUpdate = list_data.state != mylistState;
      mylistEdit = true;
    } catch (_e: unknown) {
      const e = _e as AnidbError;
      switch (e.code) {
        case 321: // NO_SUCH_ENTRY
          needUpdate = true;
          mylistEdit = false;
          break;
        default:
          await this.disconnect();
          throw _e as Error;
      }
    }

    if (needUpdate) {
      const update_data = await this.anidb.mylist_add_by_file(
        { ed2k: episode.ed2khash.hash, size: episode.ed2khash.size },
        {
          edit: mylistEdit,
          state: mylistState,
        },
      );

      if (
        update_data?.code != "MYLIST_ENTRY_ADDED" &&
        update_data?.code != "MYLIST_ENTRY_EDITED"
      )
        return false;
    }

    return true;
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
