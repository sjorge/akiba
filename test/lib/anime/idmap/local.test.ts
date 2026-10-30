import fs from "node:fs";
import path from "node:path";
import { expect, test } from "bun:test";

import type { AnimeId } from "lib/anime";

import { AnimeIdmapLocal } from "lib/anime/idmap/local";
import { makeConfig, seedCacheFile } from "test/helpers";

test("AnimeIdmapLocal refreshes from a file and applies the mapping", async (): Promise<void> => {
  // missing and invalid files
  const missing = makeConfig();
  expect(await new AnimeIdmapLocal(missing).refresh()).toBe(false);
  fs.writeFileSync(path.join(missing.cache.path, "local.map.toml"), "[[[");
  expect(await new AnimeIdmapLocal(missing).refresh()).toBe(false);

  const config = makeConfig();
  seedCacheFile(config.cache.path, "local.map.toml", {
    "1": { anilist: 10, tmdb: 20, tmdbSeason: 2 },
  });
  const idmap = new AnimeIdmapLocal(config);
  expect(await idmap.refresh()).toBe(true);

  const empty: AnimeId = { anidb: 1 };
  idmap.apply(empty);
  expect(empty).toEqual({ anidb: 1, anilist: 10, tmdb: 20, tmdbSeason: 2 });

  const existing: AnimeId = { anidb: 1, anilist: 99, tmdb: 98 };
  idmap.apply(existing);
  expect(existing).toEqual({ anidb: 1, anilist: 99, tmdb: 98 });
  idmap.apply(existing, true);
  expect(existing).toEqual({ anidb: 1, anilist: 10, tmdb: 20, tmdbSeason: 2 });

  const unknown: AnimeId = { anidb: 2 };
  idmap.apply(unknown);
  expect(unknown).toEqual({ anidb: 2 });
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
