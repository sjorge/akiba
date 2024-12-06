/*
 * Internal helper types and function for anidb
 */
import fs from "node:fs";
import path from "node:path";
import { createMD4 } from "hash-wasm";

import type { Config } from "lib/config";

export class AnimeFormatStringException extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AnimeFormatStringException";
  }
}

export type AnimeFormatStringData = {
  fid: string;
  aid: string;
  eid: string;
  gid: string;
  lid: string;
  status: string;
  size: string;
  ed2k: string;
  md5: string;
  sha1: string;
  crc32: string;
  lang_dub: string;
  lang_sub: string;
  quaility: string;
  source: string;
  audio_codec: string;
  audio_bitrate: string;
  video_codec: string;
  video_bitrate: string;
  resolution: string;
  filetype: string;
  length: string;
  description: string;
  group: string;
  group_short: string;
  episode: string;
  episode_name: string;
  episode_name_romaji: string;
  episode_name_kanji: string;
  episode_total: string;
  episode_last: string;
  anime_year: string;
  anime_type: string;
  anime_name_romaji: string;
  anime_name_kanji: string;
  anime_name_english: string;
  anime_name_other: string;
  anime_name_short: string;
  anime_synonyms: string;
  anime_category: string;
  version: string;
  censored: string;
  orginal_name: string;
};

export function animeFormatString(
  template: string,
  data: AnimeFormatStringData,
): string {
  return template.replace(
    /{(\w+)}/g,
    (match: string, tag: keyof AnimeFormatStringData) => {
      if (data[tag] === undefined)
        throw new AnimeFormatStringException(
          `The tag ${match} is not known, available: ${Object.keys(data).join(", ")}.`,
        );

      return data[tag];
    },
  );
}

export function animeFormatValidate(
  template: string,
  thowException: boolean = false,
): boolean {
  try {
    animeFormatString(template, {
      fid: "",
      aid: "",
      eid: "",
      gid: "",
      lid: "",
      status: "",
      size: "",
      ed2k: "",
      md5: "",
      sha1: "",
      crc32: "",
      lang_dub: "",
      lang_sub: "",
      quaility: "",
      source: "",
      audio_codec: "",
      audio_bitrate: "",
      video_codec: "",
      video_bitrate: "",
      resolution: "",
      filetype: "",
      length: "",
      description: "",
      group: "",
      group_short: "",
      episode: "",
      episode_name: "",
      episode_name_romaji: "",
      episode_name_kanji: "",
      episode_total: "",
      episode_last: "",
      anime_year: "",
      anime_type: "",
      anime_name_romaji: "",
      anime_name_kanji: "",
      anime_name_english: "",
      anime_name_other: "",
      anime_name_short: "",
      anime_synonyms: "",
      anime_category: "",
      version: "",
      censored: "",
      orginal_name: "",
    });
  } catch (_e: unknown) {
    if (thowException) throw _e;
    return false;
  }

  return true;
}

type Ed2kHash = {
  hash: string;
  size: number;
  link: string;
  timestamp: number;
};

export async function generateEd2kHash(file: string): Promise<Ed2kHash> {
  // sanity check parameters
  if (!fs.existsSync(file)) throw new Error(`${file} does not exist!`);
  if (!fs.statSync(file).isFile()) throw new Error(`${file} must be a file!`);

  async function hash(file: string): Promise<string> {
    // https://wiki.anidb.net/Ed2k-hash
    const md4 = await createMD4();
    const blockSize = 9728000;
    const blockHashes: string[] = [];
    let buffer = Buffer.alloc(0);

    const fileStream = fs.createReadStream(file);

    for await (const chunk of fileStream) {
      buffer = Buffer.concat([buffer, chunk]);

      // Process full blocks
      while (buffer.length >= blockSize) {
        const block = buffer.subarray(0, blockSize);
        buffer = buffer.subarray(blockSize);

        md4.init();
        md4.update(block);

        blockHashes.push(md4.digest());
      }
    }

    // Process remaining data (last chunk)
    if (buffer.length > 0) {
      md4.init();
      md4.update(buffer);
      blockHashes.push(md4.digest());
    }

    // return fast as we have a single block
    if (blockHashes.length === 1) return blockHashes[0];

    // compute hash of all block hashes if we have multiple
    md4.init();
    for (const hash of blockHashes) {
      md4.update(Buffer.from(hash, "hex"));
    }
    return md4.digest();
  }

  // generate hash
  const fileHash: string = await hash(file);
  const fileSize: number = fs.statSync(file).size;

  return {
    hash: fileHash,
    size: fileSize,
    link: `ed2k://|file|${path.basename(file)}|${fileSize}|${fileHash}|/`,
    timestamp: 0,
  } as Ed2kHash;
}

export class AnimeRenamer {
  private format: string;
  private rehash: boolean;

  public constructor(config: Config, rehash: boolean = false, format?: string) {
    this.format = config.renamer.format;
    this.rehash = rehash;
    if (format) this.format = format;
  }

  public toString(): string {
    return "AnimeRenamer";
  }

  public async identify(animeEpisodeFile: string): Promise<void> {
    const episodeHash = await generateEd2kHash(animeEpisodeFile);
    console.log(
      `${animeEpisodeFile}: size=${episodeHash.size} hash=${episodeHash.hash}`,
    );
  }

  public async rename(
    animeEpisodeFile: string,
    metadata: AniDB_Show,
    overwrite: boolean = false,
    symlink: boolean = true,
  ): Promise<void> {
    console.log(
      `TODO ${symlink ? "symlink" : "move"} ${animeEpisodeFile} using metadata ${metadata}, overwrite = ${overwrite}`,
    );
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
