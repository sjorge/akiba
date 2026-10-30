import fs from "node:fs";
import path from "node:path";
import { expect, test } from "bun:test";

import type { AnimeId } from "lib/anime";
import type { EpisodeFile } from "lib/anime/resolver";

import { AnimeEpisodeNfo, AnimeShowNfo } from "lib/anime/nfo";
import { makeShow } from "test/fixtures/anidb-show";
import { makeTempDir } from "test/helpers";

function showNfo(
  id: AnimeId = { anidb: 100 },
  overrides: Partial<AniDB_Show> = {},
): AnimeShowNfo {
  return new AnimeShowNfo(id, makeShow(overrides), makeTempDir());
}

function episodeNfo(start: string, end: string): AnimeEpisodeNfo {
  const file = path.join(makeTempDir(), "Show - 01 - Title.mkv");
  fs.writeFileSync(file, "");
  const episodeFile: EpisodeFile = {
    path: file,
    episodeStart: start,
    episodeEnd: end,
    title: "File `Title`",
  };
  return new AnimeEpisodeNfo({ anidb: 100 }, episodeFile, makeShow());
}

test("AnimeShowNfo maps AniDB metadata to a tvshow", (): void => {
  const nfo = showNfo();

  // NOTE: regression for replaceAll, both backticks must be replaced
  expect(nfo.tvShow.title).toBe("Test 'Anime'");
  expect(nfo.tvShow.originaltitle).toBe("テスト");
  expect(nfo.tvShow.uniqueId).toEqual([
    { type: "anidb", id: 100, default: true },
  ]);
  expect(nfo.tvShow.premiered).toBe("2020-01-01");
  expect(nfo.tvShow.poster).toBe("https://cdn.anidb.net/images/main/12345.jpg");
  expect(nfo.tvShow.mpaa).toBe("G");
  expect(nfo.isValid()).toBe(true);

  const ids = showNfo({ anidb: 100, anilist: 5, tmdb: 6 }).tvShow.uniqueId;
  expect(ids.map((id) => [id.type, id.id])).toEqual([
    ["anidb", 100],
    ["anilist", 5],
    ["tmdb", 6],
  ]);
  expect(
    showNfo({ anidb: 100, anilist: -1, tmdb: -1 }).tvShow.uniqueId,
  ).toHaveLength(1);

  expect(
    showNfo({ anidb: 100 }, { startDate: "2020-02-03" }).tvShow.premiered,
  ).toBe("2020-02-03");
  expect(
    showNfo({ anidb: 100 }, { startDate: "2020" }).tvShow.premiered,
  ).toBeUndefined();
  expect(showNfo({ anidb: 100 }, { ageRestricted: true }).tvShow.mpaa).toBe(
    "18+",
  );
});

test("AnimeEpisodeNfo maps AniDB episodes", (): void => {
  const [first, ...rest] = episodeNfo("1", "1").episode;
  expect(rest).toHaveLength(0);
  expect(first.season).toBe(1);
  expect(first.episode).toBe(1);
  expect(first.uniqueId).toEqual([{ type: "anidb", id: 1001, default: true }]);
  // en wins over the file title, ja is the original title
  expect(first.title).toBe("First 'Ep'");
  expect(first.originaltitle).toBe("第一話");

  // a range is expanded, with the file title as a fallback and the airdate
  // padded or left out
  const range = episodeNfo("1", "3").episode;
  expect(range.map((e) => e.episode)).toEqual([1, 2, 3]);
  expect(range.map((e) => e.premiered)).toEqual([
    "2020-01-08",
    "2020-01-01",
    undefined,
  ]);
  expect(range[2].title).toBe("Third");

  // a special is season 0 with type * 100 + n
  const [special] = episodeNfo("S1", "S1").episode;
  expect(special.season).toBe(0);
  expect(special.episode).toBe(201);

  expect(episodeNfo("9", "9").episode).toEqual([]);
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
