import { expect, test } from "bun:test";

import type { AnimeId } from "lib/anime";

import { AnimeIdmapList } from "lib/anime/idmap/list";
import { makeConfig, seedCacheFile } from "test/helpers";

test("AnimeIdmapList loads a fresh cache and applies the mapping", async (): Promise<void> => {
  const config = makeConfig();
  seedCacheFile(config.cache.path, "list.map.toml", { "1": { anilist: 10 } });
  const idmap = new AnimeIdmapList(config);
  // NOTE: a fresh cache file is loaded without touching the network
  expect(await idmap.refresh()).toBe(true);

  const empty: AnimeId = { anidb: 1 };
  idmap.apply(empty);
  expect(empty).toEqual({ anidb: 1, anilist: 10 });

  const existing: AnimeId = { anidb: 1, anilist: 99 };
  idmap.apply(existing);
  expect(existing.anilist).toBe(99);
  idmap.apply(existing, true);
  expect(existing.anilist).toBe(10);

  const unknown: AnimeId = { anidb: 2 };
  idmap.apply(unknown);
  expect(unknown).toEqual({ anidb: 2 });
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
