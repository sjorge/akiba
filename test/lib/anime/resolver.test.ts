import fs from "node:fs";
import path from "node:path";
import { expect, test } from "bun:test";

import { AnimeResolver } from "lib/anime/resolver";
import { makeConfig, makeTempDir, seedCacheFile } from "test/helpers";

test("AnimeResolver resolves ids and titles from the cache", async (): Promise<void> => {
  const config = makeConfig();
  seedCacheFile(config.cache.path, "titles.map.toml", {
    "1": [
      { title: "Cowboy Bebop", type: "main", language: "x-jat" },
      { title: "Cowboy Bebop (EN)", type: "official", language: "en" },
    ],
    "2": [{ title: "Narutxyz", type: "main", language: "x-jat" }],
    "3": [{ title: "Naruto", type: "main", language: "x-jat" }],
    "4": [{ title: "Fate/Zero", type: "main", language: "x-jat" }],
  });
  const resolver = new AnimeResolver(config);
  // NOTE: a fresh cache file is loaded without touching the network
  await resolver.refresh();

  expect(resolver.id("Whatever [anidb-123]")).toEqual({ anidb: 123 });
  expect(resolver.id("Cowboy Bebop")).toEqual({ anidb: 1 });
  expect(resolver.id("Cowboy Bebap")).toEqual({ anidb: 1 });
  expect(resolver.id("Narutx")).toEqual({ anidb: 3 });
  expect(resolver.id("Fate⁄Zero")).toEqual({ anidb: 4 });
  expect(resolver.id("Completely Different")).toBeUndefined();

  expect(resolver.title({ anidb: 1 }).map((t) => t.title)).toEqual([
    "Cowboy Bebop",
    "Cowboy Bebop (EN)",
  ]);
  expect(resolver.title({ anidb: 999 })).toBeUndefined();
});

test("AnimeResolver.episodes parses a directory of episode files", (): void => {
  const dir = makeTempDir();
  for (const file of [
    "Show - 01 - First.mkv",
    "Show - 2 - Second (DEADBEEF).mkv",
    "Show - 10 - Tenth.mp4",
    "Show - S1 - Special.mkv",
    "Show - C2 - Credits.avi",
    "Show - 01-03 - Multi.mkv",
    "Show - 04 - Not Video.txt",
    "garbage.mkv",
  ]) {
    fs.writeFileSync(path.join(dir, file), "");
  }
  const resolver = new AnimeResolver(makeConfig());

  // natural order, leading zeros stripped, specials kept, checksum removed,
  // and non video or unparseable names skipped
  const episodes = resolver.episodes(dir);
  expect(episodes.map((e) => [e.episodeStart, e.episodeEnd, e.title])).toEqual([
    ["1", "1", "First"],
    ["1", "3", "Multi"],
    ["2", "2", "Second"],
    ["10", "10", "Tenth"],
    ["C2", "C2", "Credits"],
    ["S1", "S1", "Special"],
  ]);
  expect(episodes[0].path).toBe(path.join(dir, "Show - 01 - First.mkv"));

  expect(resolver.episodes(path.join(dir, "missing"))).toEqual([]);
  expect(resolver.episodes(path.join(dir, "garbage.mkv"))).toEqual([]);
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
