import fs from "node:fs";
import path from "node:path";
import { expect, spyOn, test } from "bun:test";
import toml from "@iarna/toml";
import { machineIdSync } from "node-machine-id";

import type { Config } from "lib/config";
import type { AnimeStringFormatData } from "lib/anime/formatter";
import type { AnimeRenamerEpisode } from "lib/anime/renamer";

import { AnidbError, AnidbUDPClient, RESPONSE_CODE } from "lib/anidb";
import { AnimeRenamer, localUdpPort } from "lib/anime/renamer";
import {
  makeConfig,
  makeTempDir,
  rejectionMessage,
  sampleFormatData,
  seedCacheFile,
} from "test/helpers";

const FORMAT = "{anime_name_romaji}/{episode:number}.{filetype}";

function clientConfig(): Config {
  return makeConfig({
    anidb: {
      udp_client: { name: "client", version: 1, username: "u", password: "p" },
    },
  });
}

// NOTE: the destination path is built from these, the rest of the episode is
//       the real record from test/fixtures
const PINNED: Partial<AnimeStringFormatData> = {
  anime_name_romaji: "Test Anime",
  episode: "013",
};

type Fixture = {
  source: string;
  target: string;
  destination: string;
  episode: (
    data?: Partial<AnimeStringFormatData>,
    renamer?: AnimeRenamer,
  ) => Promise<AnimeRenamerEpisode>;
  renamer: AnimeRenamer;
};

// a fresh source file, an empty target and a renamer for them
function fixture(format: string = FORMAT, target?: string): Fixture {
  const dir = makeTempDir();
  const source = path.join(dir, "source.mkv");
  const targetDir = path.join(dir, "target");
  fs.mkdirSync(targetDir);
  fs.writeFileSync(source, "abc");

  const renamer = new AnimeRenamer(
    clientConfig(),
    false,
    format,
    target ?? targetDir,
  );
  const result: Fixture = {
    source,
    target: targetDir,
    destination: path.join(targetDir, "Test Anime", "13.mkv"),
    renamer,
    episode: async (
      data = {},
      other = renamer,
    ): Promise<AnimeRenamerEpisode> => {
      // NOTE: reads result.source so a test can point it somewhere else
      const episode = await other.identify(result.source, false, true);
      episode.data = sampleFormatData({ ...PINNED, ...data });
      return episode;
    },
  };
  return result;
}

test("AnimeRenamer validates its config and picks a stable port", (): void => {
  expect(localUdpPort("host-a")).toBe(localUdpPort("host-a"));
  expect(localUdpPort("host-a")).not.toBe(localUdpPort("host-b"));
  for (const host of ["a", "b", "c", "localhost", "some.long.hostname"]) {
    expect(localUdpPort(host)).toBeGreaterThanOrEqual(10000);
    expect(localUdpPort(host)).toBeLessThan(60000);
  }

  expect((): AnimeRenamer => new AnimeRenamer(makeConfig())).toThrow(
    "udp_client",
  );
  for (const missing of ["name", "username", "password"]) {
    const config = clientConfig();
    delete (config.anidb.udp_client as Record<string, unknown>)[missing];
    expect((): AnimeRenamer => new AnimeRenamer(config)).toThrow();
  }

  // NOTE: the constructor does not open a connection
  expect(new AnimeRenamer(clientConfig()).toString()).toBe("AnimeRenamer");
});

test("AnimeRenamer.identify hashes and caches without the network", async (): Promise<void> => {
  const { source, renamer } = fixture();

  const first = await renamer.identify(source, false, true);
  expect(first.path).toBe(source);
  expect(first.ed2khash.hash).toBe("a448017aaf21d8525fc10ae87aa6729d");
  expect(first.data).toBeUndefined();

  const second = await renamer.identify(source, false, true);
  expect(second.ed2khash.createdAt).toBe(first.ed2khash.createdAt);

  expect(
    await rejectionMessage(renamer.identify("/nonexistent/x.mkv", false, true)),
  ).toContain("does not exist");
});

// the hash cache file of this machine, there is one per config
function hashCacheFile(config: Config): string {
  const names = fs
    .readdirSync(config.cache.path)
    .filter((name: string): boolean => name.endsWith(".hashes.toml"));
  expect(names.length).toBe(1);
  return path.join(config.cache.path, names[0]);
}

test("AnimeRenamer stores absolute hash keys for a relative target", async (): Promise<void> => {
  // NOTE: regression for a relative target storing relative hash cache keys
  const f = fixture();
  const config = clientConfig();
  const cwd = process.cwd();
  process.chdir(path.dirname(f.target));
  try {
    const relative = new AnimeRenamer(config, false, FORMAT, "target");
    const episode = await f.episode({}, relative);
    const createdAt = episode.ed2khash.createdAt;

    const result = await relative.rename(episode);
    expect(result.action).toBe("move");

    // NOTE: process.cwd() is the real path, /var is a link to /private/var
    const destination = result.destination_path as string;
    expect(destination).toBe(
      path.join(process.cwd(), "target", "Test Anime", "13.mkv"),
    );
    const cache = toml.parse(fs.readFileSync(hashCacheFile(config), "utf8"));
    expect(Object.keys(cache)).toEqual([destination]);

    // a new renamer finds the entry by its absolute path, without rehashing
    await Bun.sleep(5);
    const again = new AnimeRenamer(config);
    const cached = await again.identify(destination, false, true);
    expect(cached.ed2khash.createdAt).toBe(createdAt);
  } finally {
    process.chdir(cwd);
  }
});

test("AnimeRenamer drops relative keys from an old hash cache", async (): Promise<void> => {
  // NOTE: regression for relative keys written before the target was resolved
  const f = fixture();
  const config = clientConfig();
  const cwd = process.cwd();
  process.chdir(path.dirname(f.target));
  try {
    // "source.mkv" exists relative to the cwd, so only its relative form
    // can get it purged
    const entry = {
      hash: "a448017aaf21d8525fc10ae87aa6729d",
      size: 3,
      link: "",
      createdAt: Date.now(),
    };
    seedCacheFile(config.cache.path, `${machineIdSync()}.hashes.toml`, {
      [f.source]: entry,
      "source.mkv": entry,
    });

    new AnimeRenamer(config);
    const cache = toml.parse(fs.readFileSync(hashCacheFile(config), "utf8"));
    expect(Object.keys(cache)).toEqual([f.source]);
  } finally {
    process.chdir(cwd);
  }
});

test("AnimeRenamer.rename moves, copies, links and skips", async (): Promise<void> => {
  // without data nothing happens
  let f = fixture();
  const bare = await f.renamer.identify(f.source, false, true);
  expect((await f.renamer.rename(bare)).action).toBeUndefined();

  // a dry run changes nothing
  f = fixture();
  let result = await f.renamer.rename(
    await f.episode(),
    false,
    false,
    false,
    true,
  );
  expect(result.action).toBe("move");
  expect(result.destination_path).toBe(f.destination);
  expect(fs.existsSync(f.source)).toBe(true);
  expect(fs.readdirSync(f.target)).toEqual([]);

  // move, with the destination directory created as 0o750
  // NOTE: regression for creating destination directories with mode 0o750
  f = fixture();
  result = await f.renamer.rename(await f.episode());
  expect(result.action).toBe("move");
  expect(fs.existsSync(f.source)).toBe(false);
  expect(fs.readFileSync(f.destination, "utf8")).toBe("abc");
  expect(fs.statSync(path.dirname(f.destination)).mode & 0o777).toBe(
    0o750 & ~process.umask(),
  );

  // copy
  f = fixture();
  result = await f.renamer.rename(await f.episode(), false, true);
  expect(result.action).toBe("copy");
  expect(fs.existsSync(f.source)).toBe(true);
  expect(fs.existsSync(f.destination)).toBe(true);

  // symlink
  f = fixture();
  result = await f.renamer.rename(await f.episode(), false, false, true);
  expect(result.action).toBe("symlink");
  expect(fs.lstatSync(f.source).isSymbolicLink()).toBe(true);
  expect(fs.readlinkSync(f.source)).toBe(f.destination);

  // a file already at the destination is uptodate
  f = fixture();
  fs.mkdirSync(path.dirname(f.destination));
  fs.renameSync(f.source, f.destination);
  f.source = f.destination;
  result = await f.renamer.rename(await f.episode());
  expect(result.action).toBe("uptodate");

  // an existing destination is kept, unless overwrite is set
  f = fixture();
  fs.mkdirSync(path.dirname(f.destination));
  fs.writeFileSync(f.destination, "existing");
  result = await f.renamer.rename(await f.episode());
  expect(result.action).toBeUndefined();
  expect(fs.existsSync(f.source)).toBe(true);
  expect(fs.readFileSync(f.destination, "utf8")).toBe("existing");
  await f.renamer.rename(await f.episode(), true);
  expect(fs.readFileSync(f.destination, "utf8")).toBe("abc");
  expect(fs.existsSync(f.source)).toBe(false);
});

test("AnimeRenamer.rename copy leaves the source hash record alone", async (): Promise<void> => {
  // NOTE: regression for a copy sharing one hash record between source and destination
  const f = fixture();
  const episode = await f.episode();
  const sourceLink = episode.ed2khash.link;
  expect(sourceLink).toContain("|source.mkv|");

  const result = await f.renamer.rename(episode, false, true);
  expect(result.action).toBe("copy");

  const source = await f.renamer.identify(f.source, false, true);
  expect(source.ed2khash.link).toBe(sourceLink);
  expect(source.ed2khash.hash).toBe(episode.ed2khash.hash);

  const copy = await f.renamer.identify(f.destination, false, true);
  expect(copy.ed2khash.link).toContain("|13.mkv|");
  expect(copy.ed2khash.hash).toBe(episode.ed2khash.hash);
});

test("AnimeRenamer.rename handles moves across devices", async (): Promise<void> => {
  // only the move of `source` fails, the cache writes rename as usual
  function crossDevice(source: string): ReturnType<typeof spyOn> {
    const real = fs.renameSync;
    return spyOn(fs, "renameSync").mockImplementation((from, to): void => {
      if (from !== source) return real(from, to);
      const e = new Error(
        "EXDEV: cross-device link not permitted",
      ) as NodeJS.ErrnoException;
      e.code = "EXDEV";
      throw e;
    });
  }

  // NOTE: regression for detecting EXDEV by error code
  let f = fixture();
  let spy = crossDevice(f.source);
  try {
    expect((await f.renamer.rename(await f.episode())).action).toBe("move");
  } finally {
    spy.mockRestore();
  }
  expect(fs.existsSync(f.source)).toBe(false);
  expect(fs.existsSync(f.destination)).toBe(true);

  // a dry run does not copy
  f = fixture();
  spy = crossDevice(f.source);
  try {
    await f.renamer.rename(await f.episode(), false, false, false, true);
  } finally {
    spy.mockRestore();
  }
  expect(fs.existsSync(f.source)).toBe(true);
  expect(fs.readdirSync(f.target)).toEqual([]);

  // other errors are rethrown
  f = fixture();
  const source = f.source;
  const real = fs.renameSync;
  const failing = spyOn(fs, "renameSync").mockImplementation(
    (from, to): void => {
      if (from !== source) return real(from, to);
      throw new Error("EACCES: permission denied");
    },
  );
  try {
    expect(
      await rejectionMessage(f.renamer.rename(await f.episode())),
    ).toContain("EACCES");
  } finally {
    failing.mockRestore();
  }
});

test("AnimeRenamer.rename keeps the file times of moves, copies and cross-device moves", async (): Promise<void> => {
  const episodeTime = new Date(1_500_000_000_000);
  const sidecarTime = new Date(1_400_000_000_000);
  const mtimeOf = (file: string): number => fs.statSync(file).mtime.getTime();
  function setTimes(f: Fixture): void {
    fs.utimesSync(f.source, episodeTime, episodeTime);
    fs.utimesSync(inSourceDir(f, "source.nfo"), sidecarTime, sidecarTime);
  }

  // a move on one file system is a plain rename
  let f = fixture();
  writeSidecars(f, ["source.nfo"]);
  setTimes(f);
  await f.renamer.rename(await f.episode());
  expect(mtimeOf(f.destination)).toBe(episodeTime.getTime());
  expect(mtimeOf(inDestinationDir(f, "13.nfo"))).toBe(sidecarTime.getTime());

  // a copy gives the episode and its sidecar the times of their sources
  f = fixture();
  writeSidecars(f, ["source.nfo"]);
  setTimes(f);
  await f.renamer.rename(await f.episode(), false, true);
  expect(mtimeOf(f.destination)).toBe(episodeTime.getTime());
  expect(mtimeOf(inDestinationDir(f, "13.nfo"))).toBe(sidecarTime.getTime());
  expect(mtimeOf(f.source)).toBe(episodeTime.getTime());

  // so does the copy fallback of a move across devices
  f = fixture();
  writeSidecars(f, ["source.nfo"]);
  setTimes(f);
  const sidecar = inSourceDir(f, "source.nfo");
  const real = fs.renameSync;
  const spy = spyOn(fs, "renameSync").mockImplementation((from, to): void => {
    if (from !== f.source && from !== sidecar) return real(from, to);
    const e = new Error("EXDEV: cross-device link") as NodeJS.ErrnoException;
    e.code = "EXDEV";
    throw e;
  });
  try {
    await f.renamer.rename(await f.episode());
  } finally {
    spy.mockRestore();
  }
  expect(fs.existsSync(f.source)).toBe(false);
  expect(mtimeOf(f.destination)).toBe(episodeTime.getTime());
  expect(mtimeOf(inDestinationDir(f, "13.nfo"))).toBe(sidecarTime.getTime());
});

test("AnimeRenamer.rename keeps destinations inside the target", async (): Promise<void> => {
  // NOTE: regression for refusing destinations outside the target path
  const dots = { anime_name_romaji: ".." };

  // a .. tag value, both in the middle and as the basename
  for (const format of [
    "{anime_name_romaji}/{anime_name_romaji}",
    "a/{anime_name_romaji}",
  ]) {
    const f = fixture(format);
    expect(
      await rejectionMessage(f.renamer.rename(await f.episode(dots))),
    ).toContain("outside of");
    expect(fs.existsSync(f.source)).toBe(true);
  }

  // a template that climbs out is neutralised by the formatter
  let f = fixture("../escaped.{filetype}");
  await f.renamer.rename(await f.episode());
  expect(fs.existsSync(path.join(f.target, "escaped.mkv"))).toBe(true);
  expect(fs.existsSync(path.join(path.dirname(f.target), "escaped.mkv"))).toBe(
    false,
  );

  // the target itself may be relative
  f = fixture();
  const relative = new AnimeRenamer(
    clientConfig(),
    false,
    FORMAT,
    path.join(f.target, "nested", "..", "renamed"),
  );
  const result = await relative.rename(await f.episode({}, relative));
  expect(result.action).toBe("move");
  expect(
    fs.existsSync(path.join(f.target, "renamed", "Test Anime", "13.mkv")),
  ).toBe(true);
});

test("AnimeRenamer.rename shortens an episode name that is too long", async (): Promise<void> => {
  const f = fixture("{anime_name_romaji}/{episode_name}.{filetype}");
  const result = await f.renamer.rename(
    await f.episode({ episode_name: "a".repeat(300) }),
  );
  const destination = result.destination_path as string;
  expect(result.action).toBe("move");
  expect(path.basename(destination).endsWith("~.mkv")).toBe(true);
  expect(fs.existsSync(f.source)).toBe(false);
  expect(fs.existsSync(destination)).toBe(true);
});

test("AnimeRenamer.rename swaps a long anime name for its short name", async (): Promise<void> => {
  const f = fixture(
    "{anime_name_romaji}/{anime_name_romaji} - {episode_name}.{filetype}",
  );
  const result = await f.renamer.rename(
    await f.episode({
      anime_name_romaji: "a".repeat(300),
      anime_name_short: ["", "Shorty", "Longer one"],
      episode_name: "Name",
    }),
  );
  expect(result.destination_path).toBe(
    path.join(f.target, "Shorty", "Shorty - Name.mkv"),
  );

  // both long: short name and a cut episode name at once
  const both = fixture(
    "{anime_name_romaji}/{anime_name_romaji} - {episode_name}.{filetype}",
  );
  const bothResult = await both.renamer.rename(
    await both.episode({
      anime_name_romaji: "a".repeat(300),
      anime_name_short: ["Shorty"],
      episode_name: "b".repeat(300),
    }),
  );
  const name = path.basename(bothResult.destination_path as string);
  expect(name.startsWith("Shorty - b")).toBe(true);
  expect(name.endsWith("~.mkv")).toBe(true);
});

test("AnimeRenamer.rename falls back to Episode N", async (): Promise<void> => {
  // an episode name of 255 bytes cannot be cut to fit with a long anime name
  const f = fixture("{anime_name_romaji} - {episode_name}.{filetype}");
  const result = await f.renamer.rename(
    await f.episode({
      anime_name_romaji: "a".repeat(235),
      anime_name_short: [],
      episode: "013",
      episode_name: "b".repeat(100),
    }),
  );
  expect(path.basename(result.destination_path as string)).toBe(
    `${"a".repeat(235)} - Episode 13.mkv`,
  );
});

test("AnimeRenamer.identify returns no data for a hash unknown to AniDB", async (): Promise<void> => {
  const f = fixture();
  const config = clientConfig();
  const renamer = new AnimeRenamer(config, false, FORMAT, f.target);
  const connect = spyOn(AnidbUDPClient.prototype, "connect").mockResolvedValue(
    undefined,
  );
  const disconnect = spyOn(
    AnidbUDPClient.prototype,
    "disconnect",
  ).mockResolvedValue(undefined);
  const lookup = spyOn(
    AnidbUDPClient.prototype,
    "file_by_hash",
  ).mockRejectedValue(new AnidbError(RESPONSE_CODE.NO_SUCH_FILE));
  try {
    const episode = await renamer.identify(f.source);
    expect(episode.data).toBeUndefined();
    expect(episode.ed2khash).toBeDefined();
    expect(lookup).toHaveBeenCalledTimes(1);
    expect(disconnect).not.toHaveBeenCalled();
    const cacheDir = path.join(config.cache.path, "anidb");
    const written = fs.existsSync(cacheDir)
      ? fs.readdirSync(cacheDir).filter((n) => n.endsWith(".fid.toml"))
      : [];
    expect(written).toEqual([]);
  } finally {
    connect.mockRestore();
    disconnect.mockRestore();
    lookup.mockRestore();
  }
});

test("AnimeRenamer.identify disconnects and rethrows on other AniDB errors", async (): Promise<void> => {
  const f = fixture();
  const connect = spyOn(AnidbUDPClient.prototype, "connect").mockResolvedValue(
    undefined,
  );
  const disconnect = spyOn(
    AnidbUDPClient.prototype,
    "disconnect",
  ).mockResolvedValue(undefined);
  const lookup = spyOn(
    AnidbUDPClient.prototype,
    "file_by_hash",
  ).mockRejectedValue(new AnidbError(RESPONSE_CODE.BANNED));
  try {
    await rejectionMessage(f.renamer.identify(f.source));
    expect(disconnect).toHaveBeenCalled();
  } finally {
    connect.mockRestore();
    disconnect.mockRestore();
    lookup.mockRestore();
  }
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab

// NOTE: writes one sidecar per name next to the source, each holding its name
function writeSidecars(f: Fixture, names: string[]): void {
  for (const name of names)
    fs.writeFileSync(path.join(path.dirname(f.source), name), name);
}

function inSourceDir(f: Fixture, name: string): string {
  return path.join(path.dirname(f.source), name);
}

function inDestinationDir(f: Fixture, name: string): string {
  return path.join(path.dirname(f.destination), name);
}

// the source episode under another name, so sidecars can have another stem
function renameSource(f: Fixture, name: string): void {
  const renamed = inSourceDir(f, name);
  fs.renameSync(f.source, renamed);
  f.source = renamed;
}

function sortedNames(dir: string): string[] {
  return fs.readdirSync(dir).sort();
}

test("AnimeRenamer.rename moves the sidecar nfo with the episode", async (): Promise<void> => {
  const f = fixture();
  writeSidecars(f, ["source.nfo"]);
  const result = await f.renamer.rename(await f.episode());
  expect(fs.existsSync(inSourceDir(f, "source.nfo"))).toBe(false);
  expect(fs.readFileSync(inDestinationDir(f, "13.nfo"), "utf8")).toBe(
    "source.nfo",
  );
  expect(result.sidecars).toEqual([
    {
      source: inSourceDir(f, "source.nfo"),
      destination: inDestinationDir(f, "13.nfo"),
      action: "move",
    },
  ]);
});

test("AnimeRenamer.rename reports no sidecars when there are none", async (): Promise<void> => {
  const f = fixture();
  const result = await f.renamer.rename(await f.episode());
  expect(result.sidecars ?? []).toEqual([]);
  expect(sortedNames(path.dirname(f.destination))).toEqual(["13.mkv"]);
});

test("AnimeRenamer.rename moves every kind of sidecar and keeps the tags", async (): Promise<void> => {
  const f = fixture();
  const names = [
    "source.nfo",
    "source.sub",
    "source.idx",
    "source.de.srt",
    "source.sup",
    "source.vtt",
    "source.ssa",
    "source.mks",
    "source.srt",
    "source.eng.forced.ass",
    "source.pt-BR.sdh.srt",
    // case is kept as it is on disk
    "source.EN.SRT",
    "source.Ass",
  ];
  writeSidecars(f, names);
  // not sidecars: another extension, another stem
  writeSidecars(f, ["source.txt", "source 2.nfo", "source.srt.bak"]);
  const result = await f.renamer.rename(await f.episode());
  expect(result.sidecars?.length).toBe(names.length);
  expect(result.sidecars?.every((s) => s.action === "move")).toBe(true);
  expect(sortedNames(path.dirname(f.destination))).toEqual(
    ["13.mkv", ...names.map((name) => name.replace("source", "13"))].sort(),
  );
  expect(sortedNames(path.dirname(f.source))).toEqual([
    "source 2.nfo",
    "source.srt.bak",
    "source.txt",
    "target",
  ]);
  expect(fs.readFileSync(inDestinationDir(f, "13.de.srt"), "utf8")).toBe(
    "source.de.srt",
  );
});

test("AnimeRenamer.rename requires a dot after the stem and keeps longer episodes' sidecars", async (): Promise<void> => {
  const f = fixture();
  renameSource(f, "Show - 1.mkv");
  writeSidecars(f, [
    "Show - 1.srt",
    "Show - 10.srt",
    "Show - 1 - Name.srt",
    "Show - 1.5.srt",
    "Show - 1.5.nfo",
  ]);
  // the extension of the longer episode is matched case-insensitively
  fs.writeFileSync(inSourceDir(f, "Show - 1.5.MKV"), "x");
  const result = await f.renamer.rename(await f.episode());
  expect(result.sidecars?.length).toBe(1);
  expect(fs.existsSync(inDestinationDir(f, "13.srt"))).toBe(true);
  expect(fs.existsSync(inSourceDir(f, "Show - 10.srt"))).toBe(true);
  expect(fs.existsSync(inSourceDir(f, "Show - 1 - Name.srt"))).toBe(true);
  expect(fs.existsSync(inSourceDir(f, "Show - 1.5.srt"))).toBe(true);
  expect(fs.existsSync(inSourceDir(f, "Show - 1.5.nfo"))).toBe(true);
});

test("AnimeRenamer.rename copies and links sidecars like the episode", async (): Promise<void> => {
  let f = fixture();
  writeSidecars(f, ["source.en.srt"]);
  let result = await f.renamer.rename(await f.episode(), false, true);
  expect(fs.existsSync(inSourceDir(f, "source.en.srt"))).toBe(true);
  expect(fs.existsSync(inDestinationDir(f, "13.en.srt"))).toBe(true);
  expect(result.sidecars?.[0].action).toBe("copy");

  f = fixture();
  writeSidecars(f, ["source.en.srt"]);
  result = await f.renamer.rename(await f.episode(), false, false, true);
  expect(fs.readlinkSync(inSourceDir(f, "source.en.srt"))).toBe(
    inDestinationDir(f, "13.en.srt"),
  );
  expect(fs.readFileSync(inDestinationDir(f, "13.en.srt"), "utf8")).toBe(
    "source.en.srt",
  );
  expect(result.sidecars?.[0].action).toBe("symlink");
});

test("AnimeRenamer.rename reports sidecars of a dry run and touches nothing", async (): Promise<void> => {
  const f = fixture();
  writeSidecars(f, ["source.en.srt", "source.ass"]);
  const result = await f.renamer.rename(
    await f.episode(),
    false,
    false,
    false,
    true,
  );
  expect(result.sidecars?.length).toBe(2);
  expect(result.sidecars?.every((s) => s.error === undefined)).toBe(true);
  expect(result.sidecars?.map((s) => s.destination).sort()).toEqual([
    inDestinationDir(f, "13.ass"),
    inDestinationDir(f, "13.en.srt"),
  ]);
  expect(fs.existsSync(inSourceDir(f, "source.en.srt"))).toBe(true);
  expect(fs.existsSync(inSourceDir(f, "source.ass"))).toBe(true);
  expect(fs.readdirSync(f.target)).toEqual([]);
});

test("AnimeRenamer.rename keeps an existing sidecar destination unless overwrite is set", async (): Promise<void> => {
  for (const overwrite of [false, true]) {
    const f = fixture();
    writeSidecars(f, ["source.en.srt"]);
    fs.mkdirSync(path.dirname(f.destination));
    fs.writeFileSync(inDestinationDir(f, "13.en.srt"), "existing");
    const result = await f.renamer.rename(await f.episode(), overwrite);
    const kept = fs.readFileSync(inDestinationDir(f, "13.en.srt"), "utf8");
    if (overwrite) {
      expect(result.sidecars?.length).toBe(1);
      expect(kept).toBe("source.en.srt");
      expect(fs.existsSync(inSourceDir(f, "source.en.srt"))).toBe(false);
    } else {
      expect(result.sidecars ?? []).toEqual([]);
      expect(kept).toBe("existing");
      expect(fs.existsSync(inSourceDir(f, "source.en.srt"))).toBe(true);
      // the episode itself is still renamed
      expect(result.action).toBe("move");
    }
  }
});

test("AnimeRenamer.rename leaves the sidecars of an up to date episode alone", async (): Promise<void> => {
  const f = fixture();
  fs.mkdirSync(path.dirname(f.destination));
  fs.renameSync(f.source, f.destination);
  f.source = f.destination;
  fs.writeFileSync(inDestinationDir(f, "13.nfo"), "x");
  const result = await f.renamer.rename(await f.episode());
  expect(result.action).toBe("uptodate");
  expect(result.sidecars ?? []).toEqual([]);
  expect(fs.readFileSync(inDestinationDir(f, "13.nfo"), "utf8")).toBe("x");
});

test("AnimeRenamer.rename never treats the episode as its own sidecar", async (): Promise<void> => {
  const f = fixture();
  renameSource(f, "source.mks");
  const result = await f.renamer.rename(await f.episode(), false, true);
  expect(result.sidecars ?? []).toEqual([]);
  expect(fs.existsSync(f.source)).toBe(true);
  expect(fs.existsSync(inDestinationDir(f, "13.mkv"))).toBe(true);
});

test("AnimeRenamer.rename cuts the stem of a sidecar name that is too long", async (): Promise<void> => {
  let f = fixture("{anime_name_romaji}/{episode_name}.{filetype}");
  writeSidecars(f, ["source.nfo"]);
  let result = await f.renamer.rename(
    await f.episode({ episode_name: "a".repeat(300) }),
  );
  const nfo = result.sidecars?.[0].destination as string;
  expect(path.basename(nfo).endsWith("~.nfo")).toBe(true);
  expect(nfo).toBe(
    (result.destination_path as string).replace(/\.mkv$/, ".nfo"),
  );
  expect(fs.existsSync(nfo)).toBe(true);
  expect(fs.existsSync(inSourceDir(f, "source.nfo"))).toBe(false);

  // only the stem is cut, the tags stay
  f = fixture("{anime_name_romaji}/{episode_name}.{filetype}");
  writeSidecars(f, ["source.pt-BR.sdh.srt"]);
  result = await f.renamer.rename(
    await f.episode({ episode_name: "a".repeat(250) }),
  );
  expect(path.basename(result.destination_path as string)).toBe(
    `${"a".repeat(250)}.mkv`,
  );
  const sidecar = result.sidecars?.[0];
  expect(sidecar?.error).toBeUndefined();
  expect(path.basename(sidecar?.destination as string)).toBe(
    `${"a".repeat(240)}~.pt-BR.sdh.srt`,
  );
  expect(fs.existsSync(sidecar?.destination as string)).toBe(true);
});

test("AnimeRenamer.rename moves a sidecar across devices", async (): Promise<void> => {
  const f = fixture();
  writeSidecars(f, ["source.pt-BR.sdh.srt"]);
  const sidecar = inSourceDir(f, "source.pt-BR.sdh.srt");
  const real = fs.renameSync;
  const spy = spyOn(fs, "renameSync").mockImplementation((from, to): void => {
    if (from !== sidecar) return real(from, to);
    const e = new Error("EXDEV: cross-device link") as NodeJS.ErrnoException;
    e.code = "EXDEV";
    throw e;
  });
  let result: AnimeRenamerEpisode;
  try {
    result = await f.renamer.rename(await f.episode());
  } finally {
    spy.mockRestore();
  }
  expect(result.sidecars?.[0].error).toBeUndefined();
  expect(fs.existsSync(sidecar)).toBe(false);
  expect(fs.readFileSync(inDestinationDir(f, "13.pt-BR.sdh.srt"), "utf8")).toBe(
    "source.pt-BR.sdh.srt",
  );
});

test("AnimeRenamer.rename reports a failing sidecar and keeps going", async (): Promise<void> => {
  const f = fixture();
  writeSidecars(f, ["source.nfo", "source.en.srt"]);
  const real = fs.renameSync;
  const spy = spyOn(fs, "renameSync").mockImplementation((from, to): void => {
    if (!String(from).endsWith(".srt")) return real(from, to);
    throw new Error("EACCES: permission denied");
  });
  let result: AnimeRenamerEpisode;
  try {
    result = await f.renamer.rename(await f.episode());
  } finally {
    spy.mockRestore();
  }
  // the episode is renamed and its hash record follows it
  expect(result.action).toBe("move");
  expect(fs.existsSync(f.destination)).toBe(true);
  expect(fs.existsSync(f.source)).toBe(false);
  expect(fs.existsSync(inSourceDir(f, "source.en.srt"))).toBe(true);
  expect(fs.existsSync(inSourceDir(f, "source.nfo"))).toBe(false);
  expect(result.sidecars?.length).toBe(2);
  const failed = result.sidecars?.find((s) => s.source.endsWith(".srt"));
  expect(failed?.error).toContain("EACCES");
  const moved = result.sidecars?.find((s) => s.source.endsWith(".nfo"));
  expect(moved?.error).toBeUndefined();
});
