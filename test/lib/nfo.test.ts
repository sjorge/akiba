import fs from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";
import { expect, spyOn, test } from "bun:test";
import type { Mock } from "bun:test";
import axios from "axios";
import type { AxiosResponse } from "axios";

import type { Episode, TvShow } from "lib/nfo";

import { EpisodeNfo, TvShowNfo } from "lib/nfo";
import { makeTempDir, rejectionMessage } from "test/helpers";

const POSTER_URL = "https://cdn.example.test/poster.jpg";

function mode(file: string): number {
  return fs.statSync(file).mode & 0o777;
}

// axios.get as a mock, its generic return type is not needed here
function spyGet(): Mock<(...args: unknown[]) => unknown> {
  return spyOn(axios, "get") as unknown as Mock<
    (...args: unknown[]) => unknown
  >;
}

// what axios resolves with for a streamed download of `body`
function download(body: string): AxiosResponse {
  return {
    data: Readable.from([Buffer.from(body)]),
  } as unknown as AxiosResponse;
}

// a download that sends some bytes and then fails, like a dropped connection
function brokenDownload(): AxiosResponse {
  const data = new Readable({
    read(): void {
      this.push("partial");
      this.destroy(new Error("connection reset"));
    },
  });
  return { data } as unknown as AxiosResponse;
}

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

test("premiered is exactly YYYY-MM-DD with a month and day in range", (): void => {
  const valid = (premiered: string): boolean =>
    new TvShowNfo(show({ premiered }), makeTempDir()).isValid();
  const validEpisode = (premiered: string): boolean =>
    new EpisodeNfo([episode({ premiered })], episodeFile()).isValid();

  // days-in-month is not checked, only the shape and the ranges
  for (const ok of ["2020-01-02", "1999-12-31", "2020-02-30", "2020-02-31"]) {
    expect(valid(ok), ok).toBe(true);
    expect(validEpisode(ok), ok).toBe(true);
  }
  expect(valid("")).toBe(true);

  for (const bad of [
    "2020-01",
    "2020-13-01",
    "2020-00-10",
    "2020-01-00",
    "2020-01-32",
    "2020-1-02",
    "x2020-01-02",
    "2020-01-02x",
    "2020-01-02\n",
    "2020/01/02",
    "Jan 2 2020",
  ]) {
    expect(valid(bad), JSON.stringify(bad)).toBe(false);
    expect(validEpisode(bad), JSON.stringify(bad)).toBe(false);
  }
});

test("NFO files are written atomically and keep mode 0o644 whatever the umask", async (): Promise<void> => {
  const dir = makeTempDir();
  const nfoPath = path.join(dir, "tvshow.nfo");
  const file = episodeFile();
  const episodeNfoPath = file.replace(/\.mkv$/, ".nfo");

  const previous = process.umask(0o077);
  try {
    // a stale NFO with a private mode is replaced with the shared mode
    fs.writeFileSync(nfoPath, "old", { mode: 0o600 });
    fs.writeFileSync(episodeNfoPath, "old", { mode: 0o600 });
    expect(await new TvShowNfo(show(), dir).write(true, false)).toBe(true);
    expect(await new EpisodeNfo([episode()], file).write(true)).toBe(true);
    expect(mode(nfoPath)).toBe(0o644);
    expect(mode(episodeNfoPath)).toBe(0o644);
  } finally {
    process.umask(previous);
  }
  expect(fs.readdirSync(dir).filter((name) => name.endsWith(".tmp"))).toEqual(
    [],
  );
});

test("a failed NFO replacement keeps the old NFO and leaves no tmp file", async (): Promise<void> => {
  const dir = makeTempDir();
  const nfoPath = path.join(dir, "tvshow.nfo");
  fs.writeFileSync(nfoPath, "old");
  const file = path.join(dir, "Show - 01 - First.mkv");
  fs.writeFileSync(file, "");
  const episodeNfoPath = path.join(dir, "Show - 01 - First.nfo");
  fs.writeFileSync(episodeNfoPath, "old");

  const failing = spyOn(fs, "renameSync").mockImplementation((): void => {
    throw new Error("EIO: input/output error");
  });
  try {
    expect(await new TvShowNfo(show(), dir).write(true, false)).toBe(false);
    expect(await new EpisodeNfo([episode()], file).write(true)).toBe(false);
  } finally {
    failing.mockRestore();
  }

  expect(fs.readFileSync(nfoPath, "utf8")).toBe("old");
  expect(fs.readFileSync(episodeNfoPath, "utf8")).toBe("old");
  expect(fs.readdirSync(dir).sort()).toEqual([
    "Show - 01 - First.mkv",
    "Show - 01 - First.nfo",
    "tvshow.nfo",
  ]);
});

test("TvShowNfo.writePoster replaces an existing poster only when overwriting", async (): Promise<void> => {
  const dir = makeTempDir();
  const poster = path.join(dir, "poster.jpg");
  fs.writeFileSync(poster, "OLD");
  const get = spyGet().mockImplementation(async (): Promise<AxiosResponse> =>
    download("NEW"),
  );
  try {
    const nfo = new TvShowNfo(show({ poster: POSTER_URL }), dir);

    expect(await nfo.writePoster(false)).toBe(true);
    expect(get).not.toHaveBeenCalled();
    expect(fs.readFileSync(poster, "utf8")).toBe("OLD");

    expect(await nfo.writePoster(true)).toBe(true);
    expect(get).toHaveBeenCalledTimes(1);
    expect(fs.readFileSync(poster, "utf8")).toBe("NEW");
    expect(mode(poster)).toBe(0o644);
    expect(fs.readdirSync(dir)).toEqual(["poster.jpg"]);
  } finally {
    get.mockRestore();
  }
});

test("TvShowNfo.writePoster keeps the old poster when the download fails", async (): Promise<void> => {
  const dir = makeTempDir();
  const poster = path.join(dir, "poster.jpg");
  fs.writeFileSync(poster, "OLD");
  const nfo = new TvShowNfo(show({ poster: POSTER_URL }), dir);

  const get = spyGet().mockImplementation(async (): Promise<never> => {
    throw new Error("connect ECONNREFUSED");
  });
  try {
    expect(await nfo.writePoster(true)).toBe(false);
  } finally {
    get.mockRestore();
  }
  expect(fs.readFileSync(poster, "utf8")).toBe("OLD");
  expect(fs.readdirSync(dir)).toEqual(["poster.jpg"]);

  // NOTE: the body stops part way through, the staged file must go too
  const broken = spyGet().mockImplementation(async (): Promise<AxiosResponse> =>
    brokenDownload(),
  );
  try {
    expect(await nfo.writePoster(true)).toBe(false);
  } finally {
    broken.mockRestore();
  }
  expect(fs.readFileSync(poster, "utf8")).toBe("OLD");
  expect(fs.readdirSync(dir)).toEqual(["poster.jpg"]);
});

test("TvShowNfo.writePoster reports a poster path that cannot be replaced", async (): Promise<void> => {
  const dir = makeTempDir();
  // NOTE: a directory where the poster should go cannot be renamed over
  fs.mkdirSync(path.join(dir, "poster.jpg"));
  const get = spyGet().mockImplementation(async (): Promise<AxiosResponse> =>
    download("NEW"),
  );
  try {
    const nfo = new TvShowNfo(show({ poster: POSTER_URL }), dir);
    expect(await nfo.writePoster(true)).toBe(false);
  } finally {
    get.mockRestore();
  }
  expect(fs.readdirSync(dir)).toEqual(["poster.jpg"]);
  expect(fs.statSync(path.join(dir, "poster.jpg")).isDirectory()).toBe(true);
});

test("TvShowNfo.write touches poster.jpg only when fetching and overwriting", async (): Promise<void> => {
  const dir = makeTempDir();
  const poster = path.join(dir, "poster.jpg");
  fs.writeFileSync(poster, "OLD");
  const get = spyGet().mockImplementation(async (): Promise<AxiosResponse> =>
    download("NEW"),
  );
  try {
    const nfo = (): TvShowNfo =>
      new TvShowNfo(show({ poster: POSTER_URL }), dir);

    // poster fetching disabled: never touched, even with overwrite
    expect(await nfo().write(true, false)).toBe(true);
    expect(get).not.toHaveBeenCalled();
    expect(fs.readFileSync(poster, "utf8")).toBe("OLD");

    // without overwrite an existing poster is kept
    expect(await nfo().write(false, true)).toBe(true);
    expect(get).not.toHaveBeenCalled();
    expect(fs.readFileSync(poster, "utf8")).toBe("OLD");

    expect(await nfo().write(true, true)).toBe(true);
    expect(fs.readFileSync(poster, "utf8")).toBe("NEW");
  } finally {
    get.mockRestore();
  }
});

function mtimeMs(file: string): number {
  return fs.statSync(file).mtime.getTime();
}

test("TvShowNfo.write gives tvshow.nfo the modification time of the show", async (): Promise<void> => {
  const dir = makeTempDir();
  const time = new Date(Date.UTC(2020, 0, 1, 12));
  const tvshow = path.join(dir, "tvshow.nfo");

  expect(
    await new TvShowNfo(show({ modified: time }), dir).write(false, false),
  ).toBe(true);
  expect(mtimeMs(tvshow)).toBe(time.getTime());

  // a kept tvshow.nfo keeps its content and its time
  const old = new Date(1_000_000_000_000);
  fs.writeFileSync(tvshow, "old");
  fs.utimesSync(tvshow, old, old);
  expect(
    await new TvShowNfo(
      show({ modified: new Date(Date.UTC(2021, 0, 1, 12)) }),
      dir,
    ).write(false, false),
  ).toBe(true);
  expect(fs.readFileSync(tvshow, "utf8")).toBe("old");
  expect(mtimeMs(tvshow)).toBe(old.getTime());

  // overwrite rewrites it and sets the time
  expect(
    await new TvShowNfo(
      show({ modified: new Date(Date.UTC(2021, 0, 1, 12)) }),
      dir,
    ).write(true, false),
  ).toBe(true);
  expect(mtimeMs(tvshow)).toBe(Date.UTC(2021, 0, 1, 12));
});

test("EpisodeNfo.write gives the episode NFO the modification time of its first episode", async (): Promise<void> => {
  const file = episodeFile();
  const nfoPath = file.replace(/\.mkv$/, ".nfo");
  const time = new Date(Date.UTC(2020, 0, 8, 12));

  expect(
    await new EpisodeNfo(
      [episode({ modified: time }), episode({ modified: new Date(0) })],
      file,
    ).write(),
  ).toBe(true);
  expect(mtimeMs(nfoPath)).toBe(time.getTime());

  // a kept episode NFO keeps its content and its time
  const old = new Date(1_000_000_000_000);
  fs.writeFileSync(nfoPath, "old");
  fs.utimesSync(nfoPath, old, old);
  expect(
    await new EpisodeNfo([episode({ modified: time })], file).write(false),
  ).toBe(true);
  expect(fs.readFileSync(nfoPath, "utf8")).toBe("old");
  expect(mtimeMs(nfoPath)).toBe(old.getTime());
});

test("an NFO without a modification time keeps the time of writing", async (): Promise<void> => {
  const dir = makeTempDir();
  const before = Date.now() - 1000;
  expect(await new TvShowNfo(show(), dir).write(false, false)).toBe(true);
  expect(mtimeMs(path.join(dir, "tvshow.nfo"))).toBeGreaterThan(before);
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
