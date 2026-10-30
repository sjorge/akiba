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
 * Template data for the formatter and the renamer, a real record out of the
 * AniDB file cache (test/fixtures/anidb-file.fid.toml)
 */
export const SAMPLE_FILE_FIXTURE: string = path.join(
  import.meta.dir,
  "fixtures",
  "anidb-file.fid.toml",
);

export function sampleFormatData(
  overrides: Partial<AnimeStringFormatData> = {},
): AnimeStringFormatData {
  return {
    ...(toml.parse(
      fs.readFileSync(SAMPLE_FILE_FIXTURE, "utf8"),
    ) as unknown as AnimeStringFormatData),
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
