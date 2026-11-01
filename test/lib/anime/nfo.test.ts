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

test("AnimeEpisodeNfo omits an episode number that does not parse", async (): Promise<void> => {
  const [base] = makeShow().episodes;
  // NOTE: parseInt would read "1.5" and "1x" as 1, and "S1x" as 1 too
  const cases: [string, number][] = [
    ["1.5", 1],
    ["1x", 1],
    ["S1x", 2],
    ["S", 2],
  ];
  for (const [number, type] of cases) {
    const dir = makeTempDir();
    const media = path.join(dir, "Show - 01 - Title.mkv");
    fs.writeFileSync(media, "");
    const metadata = makeShow({
      episodes: [{ ...base, episodeNumber: number, type }],
    });

    const nfo = new AnimeEpisodeNfo(
      { anidb: 100 },
      { path: media, episodeStart: number, episodeEnd: number, title: "File" },
      metadata,
    );
    expect(nfo.episode, number).toHaveLength(1);
    expect(nfo.episode[0].episode, number).toBeUndefined();
    expect(await nfo.write(), number).toBe(true);

    const xml = fs.readFileSync(media.replace(/\.mkv$/, ".nfo"), "utf8");
    expect(xml, number).not.toContain("NaN");
    expect(xml, number).not.toContain("<episode>");
    expect(xml, number).toContain("<episodedetails>");
  }
});

test("AnimeShowNfo checks the start date as a premiered date", (): void => {
  expect(showNfo({ anidb: 100 }, { startDate: "2020-02-30" }).isValid()).toBe(
    true,
  );
  expect(showNfo({ anidb: 100 }, { startDate: "2020-13-45" }).isValid()).toBe(
    false,
  );
  expect(showNfo({ anidb: 100 }, { startDate: "2020-01-0x" }).isValid()).toBe(
    false,
  );
});

test("AnimeShowNfo takes the start date as the modification time at noon UTC", (): void => {
  // a partial date is its first day, a year alone is January 1st
  expect(showNfo().tvShow.modified).toEqual(new Date(Date.UTC(2020, 0, 1, 12)));
  expect(
    showNfo({ anidb: 100 }, { startDate: "2020-02-03" }).tvShow.modified,
  ).toEqual(new Date(Date.UTC(2020, 1, 3, 12)));
  expect(
    showNfo({ anidb: 100 }, { startDate: "2020" }).tvShow.modified,
  ).toEqual(new Date(Date.UTC(2020, 0, 1, 12)));
  expect(
    showNfo({ anidb: 100 }, { startDate: "1970-01-01" }).tvShow.modified,
  ).toEqual(new Date(43_200_000));

  // a missing, malformed, impossible or early date leaves the time alone
  for (const startDate of [
    "",
    "abc",
    "2020-1-5",
    "2020-13-01",
    "2020-02-30",
    "2021-02-29",
    "2020-01-01-01",
    "1969-12-31",
    "1900",
  ]) {
    expect(
      showNfo({ anidb: 100 }, { startDate }).tvShow.modified,
      startDate,
    ).toBeUndefined();
  }
});

test("AnimeEpisodeNfo takes the air date of each episode as its modification time", (): void => {
  // the first episode of a range sets the time of the NFO
  const range = episodeNfo("1", "3").episode;
  expect(range.map((e) => e.modified)).toEqual([
    new Date(Date.UTC(2020, 0, 8, 12)),
    new Date(Date.UTC(2020, 0, 1, 12)),
    undefined,
  ]);
});

test("AnimeEpisodeNfo.write sets the NFO time from the first episode", async (): Promise<void> => {
  const dir = makeTempDir();
  const media = path.join(dir, "Show - 01-03.mkv");
  fs.writeFileSync(media, "");
  const nfo = new AnimeEpisodeNfo(
    { anidb: 100 },
    { path: media, episodeStart: "1", episodeEnd: "3", title: "File" },
    makeShow(),
  );
  expect(await nfo.write()).toBe(true);
  expect(fs.statSync(path.join(dir, "Show - 01-03.nfo")).mtime.getTime()).toBe(
    Date.UTC(2020, 0, 8, 12),
  );
});

test("AnimeShowNfo.write gives tvshow.nfo the start date", async (): Promise<void> => {
  const dir = makeTempDir();
  expect(
    await new AnimeShowNfo({ anidb: 100 }, makeShow(), dir).write(false, false),
  ).toBe(true);
  expect(fs.statSync(path.join(dir, "tvshow.nfo")).mtime.getTime()).toBe(
    Date.UTC(2020, 0, 1, 12),
  );
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
