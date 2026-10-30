import fs from "node:fs";
import path from "node:path";
import { expect, test } from "bun:test";

import type { Episode, TvShow } from "lib/nfo";

import { EpisodeNfo, TvShowNfo } from "lib/nfo";
import { makeTempDir, rejectionMessage } from "test/helpers";

function show(overrides: Partial<TvShow> = {}): TvShow {
  return {
    uniqueId: [{ id: 1, type: "anidb" }],
    title: "Test Anime",
    ...overrides,
  };
}

function episode(overrides: Partial<Episode> = {}): Episode {
  return {
    uniqueId: [{ id: 5, type: "anidb" }],
    title: "First",
    ...overrides,
  };
}

function episodeFile(): string {
  const file = path.join(makeTempDir(), "Show - 01 - First.mkv");
  fs.writeFileSync(file, "");
  return file;
}

test("TvShowNfo validates its path and data", (): void => {
  expect((): TvShowNfo => new TvShowNfo(show(), "/nonexistent")).toThrow(
    "is not a directory",
  );

  function valid(overrides: Partial<TvShow>): boolean {
    return new TvShowNfo(show(overrides), makeTempDir()).isValid();
  }

  expect(valid({ uniqueId: [] })).toBe(false);
  expect(valid({})).toBe(true);
  expect(
    valid({
      uniqueId: [
        { id: 1, type: "anidb", default: true },
        { id: 2, type: "anilist", default: true },
      ],
    }),
  ).toBe(false);
  expect(
    valid({
      uniqueId: [
        { id: 1, type: "anidb" },
        { id: 2, type: "anilist" },
      ],
    }),
  ).toBe(false);
  expect(valid({ premiered: "2020-01" })).toBe(false);
  expect(valid({ premiered: "2020-13-45" })).toBe(false);
  expect(valid({ premiered: "2020-01-02" })).toBe(true);

  // a single uniqueId becomes the default
  const nfo = new TvShowNfo(show(), makeTempDir());
  nfo.isValid();
  expect(nfo.tvShow.uniqueId[0].default).toBe(true);
});

test("TvShowNfo.write writes tvshow.nfo", async (): Promise<void> => {
  const dir = makeTempDir();
  const nfoPath = path.join(dir, "tvshow.nfo");

  const invalid = new TvShowNfo(show({ uniqueId: [] }), dir);
  expect(await rejectionMessage(invalid.write(false, false))).toContain(
    "invalid data",
  );

  const full = new TvShowNfo(
    show({
      uniqueId: [
        { id: 1, type: "anidb", default: true },
        { id: 2, type: "anilist" },
        { id: 3, type: "tmdb" },
      ],
      originaltitle: "テスト",
      sorttitle: "Sort",
      premiered: "2020-01-02",
      plot: "A plot",
      mpaa: "G",
      tag: ["Action"],
      studio: ["Studio"],
      actor: [{ name: "Name", role: "Role", order: 0 }],
    }),
    dir,
  );
  expect(await full.write(false, false)).toBe(true);

  const xml = fs.readFileSync(nfoPath, "utf8");
  for (const expected of [
    '<?xml version="1.0" encoding="utf-8"?>',
    "<title>Test Anime</title>",
    "<originaltitle>テスト</originaltitle>",
    "<sorttitle>Sort</sorttitle>",
    'type="AniDB"',
    'type="AniList"',
    'type="tmdb"',
    "<premiered>2020-01-02</premiered>",
    "<plot>A plot</plot>",
    "<mpaa>G</mpaa>",
    "<tag>Action</tag>",
    "<studio>Studio</studio>",
    "<name>Name</name>",
    "<order>0</order>",
  ]) {
    expect(xml).toContain(expected);
  }

  // existing nfo is kept unless overwrite is set
  fs.writeFileSync(nfoPath, "old");
  await new TvShowNfo(show(), dir).write(false, false);
  expect(fs.readFileSync(nfoPath, "utf8")).toBe("old");
  await new TvShowNfo(show(), dir).write(true, false);
  const minimal = fs.readFileSync(nfoPath, "utf8");
  expect(minimal).toContain("<tvshow>");
  expect(minimal).not.toContain("<originaltitle>");

  // NOTE: regression for poster failures, port 1 on loopback refuses the
  //       connection without touching the network
  const poster = new TvShowNfo(
    show({ poster: "http://127.0.0.1:1/poster.jpg" }),
    dir,
  );
  expect(await poster.write(true, true)).toBe(false);
  expect(fs.existsSync(path.join(dir, "poster.jpg"))).toBe(false);
});

test("EpisodeNfo validates its path and data", (): void => {
  for (const bad of ["/nonexistent/x.mkv", makeTempDir()]) {
    expect((): EpisodeNfo => new EpisodeNfo([episode()], bad)).toThrow(
      "is not a file",
    );
  }

  function valid(episodes: Episode[]): boolean {
    return new EpisodeNfo(episodes, episodeFile()).isValid();
  }

  expect(valid([])).toBe(false);
  expect(valid([episode()])).toBe(true);
  expect(valid([episode({ premiered: "nope" })])).toBe(false);
  expect(
    valid([
      episode({
        uniqueId: [
          { id: 1, type: "anidb", default: true },
          { id: 2, type: "anilist", default: true },
        ],
      }),
    ]),
  ).toBe(false);
});

test("EpisodeNfo.write writes several episodedetails", async (): Promise<void> => {
  const file = episodeFile();
  const nfoPath = file.replace(/\.mkv$/, ".nfo");

  const invalid = new EpisodeNfo([episode({ uniqueId: [] })], file);
  expect(await rejectionMessage(invalid.write())).toContain("invalid data");

  const nfo = new EpisodeNfo(
    [
      episode({ originaltitle: "オリジナル", season: 1, episode: 1 }),
      episode({ title: "Second", episode: 2 }),
    ],
    file,
  );
  expect(await nfo.write()).toBe(true);

  const xml = fs.readFileSync(nfoPath, "utf8");
  expect(xml.startsWith('<?xml version="1.0" encoding="utf-8"?>\n')).toBe(true);
  expect(xml.match(/<episodedetails>/g)).toHaveLength(2);
  expect(xml).toContain("<title>Second</title>");
  expect(xml).toContain("<originaltitle>オリジナル</originaltitle>");
  expect(xml).toContain("<season>1</season>");
  expect(xml).toContain('type="AniDB"');

  // existing nfo is kept unless overwrite is set
  fs.writeFileSync(nfoPath, "old");
  expect(await new EpisodeNfo([episode()], file).write(false)).toBe(true);
  expect(fs.readFileSync(nfoPath, "utf8")).toBe("old");
  await new EpisodeNfo([episode()], file).write(true);
  expect(fs.readFileSync(nfoPath, "utf8")).toContain("<episodedetails>");
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
