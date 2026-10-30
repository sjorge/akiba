/*
 * Shared helpers for the tests
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import toml from "@iarna/toml";
import { deepmerge } from "deepmerge-ts";
import { spyOn } from "bun:test";

import type { Config } from "lib/config";
import type { AnimeStringFormatData } from "lib/anime/formatter";

import { readConfig } from "lib/config";

export type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K];
};

/*
 * Create a fresh temporary directory
 */
export function makeTempDir(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), "akiba-test-"));
}

/*
 * Default configuration with its cache in a fresh temporary directory
 */
export function makeConfig(overrides: DeepPartial<Config> = {}): Config {
  const config = readConfig();
  config.cache.path = makeTempDir();
  return deepmerge(config, overrides) as Config;
}

/*
 * Write a TOML cache file with a fresh mtime
 */
export function seedCacheFile(
  dir: string,
  name: string,
  content: object,
): string {
  const file = path.join(dir, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, toml.stringify(content as toml.JsonMap));
  fs.utimesSync(file, new Date(), new Date());
  return file;
}

/*
 * Realistic template data for the formatter and the renamer
 */
export function sampleFormatData(
  overrides: Partial<AnimeStringFormatData> = {},
): AnimeStringFormatData {
  return {
    fid: 1,
    aid: 100,
    eid: 1000,
    gid: 10,
    lid: 0,
    status: 0,
    size: 1024,
    ed2k: "31d6cfe0d16ae931b73c59d7e0c089c0",
    md5: "",
    sha1: "",
    crc32: "deadbeef",
    lang_dub: "japanese",
    lang_sub: "english",
    quaility: "high",
    source: "www",
    audio_codec: "AAC",
    audio_bitrate: "128",
    video_codec: "H264",
    video_bitrate: "1000",
    resolution: "1920x1080",
    filetype: "mkv",
    length: 1440,
    description: "",
    group: "Some Group",
    group_short: "SG",
    episode: "013",
    episode_name: "The Episode",
    episode_name_romaji: "Za Episode",
    episode_name_kanji: "エピソード",
    episode_total: 24,
    episode_last: 24,
    anime_year: [2020],
    anime_type: "TV Series",
    anime_name_romaji: "Test Anime",
    anime_name_kanji: "テスト",
    anime_name_english: "Test Anime EN",
    anime_name_other: [],
    anime_name_short: [],
    anime_synonyms: [],
    anime_category: ["Action", "Comedy"],
    version: "v1",
    censored: "",
    orginal_name: "",
    ...overrides,
  };
}

/*
 * Message of the error a promise rejects with, fails if it resolves
 */
export async function rejectionMessage(
  promise: Promise<unknown>,
): Promise<string> {
  try {
    await promise;
  } catch (_e: unknown) {
    return (_e as Error).message;
  }
  throw new Error("Expected the promise to reject");
}

/*
 * Collect what `fn` writes to process.stdout
 */
export async function captureStdout(
  fn: () => void | Promise<void>,
): Promise<string> {
  let output = "";
  const spy = spyOn(process.stdout, "write").mockImplementation(
    (chunk: string | Uint8Array): boolean => {
      output += chunk.toString();
      return true;
    },
  );
  try {
    await fn();
  } finally {
    spy.mockRestore();
  }
  return output;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
