import fs from "node:fs";
import path from "node:path";
import { expect, spyOn, test } from "bun:test";

import type { Config } from "lib/config";
import type { AnimeStringFormatData } from "lib/anime/formatter";
import type { AnimeRenamerEpisode } from "lib/anime/renamer";

import { AnimeRenamer, localUdpPort } from "lib/anime/renamer";
import {
  makeConfig,
  makeTempDir,
  rejectionMessage,
  sampleFormatData,
} from "test/helpers";

const FORMAT = "{anime_name_romaji}/{episode:number}.{filetype}";

function clientConfig(): Config {
  return makeConfig({
    anidb: {
      udp_client: { name: "client", version: 1, username: "u", password: "p" },
    },
  });
}

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
      episode.data = sampleFormatData(data);
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

test("AnimeRenamer.rename handles moves across devices", async (): Promise<void> => {
  function crossDevice(): ReturnType<typeof spyOn> {
    return spyOn(fs, "renameSync").mockImplementation((): void => {
      const e = new Error(
        "EXDEV: cross-device link not permitted",
      ) as NodeJS.ErrnoException;
      e.code = "EXDEV";
      throw e;
    });
  }

  // NOTE: regression for detecting EXDEV by error code
  let f = fixture();
  let spy = crossDevice();
  try {
    expect((await f.renamer.rename(await f.episode())).action).toBe("move");
  } finally {
    spy.mockRestore();
  }
  expect(fs.existsSync(f.source)).toBe(false);
  expect(fs.existsSync(f.destination)).toBe(true);

  // a dry run does not copy
  f = fixture();
  spy = crossDevice();
  try {
    await f.renamer.rename(await f.episode(), false, false, false, true);
  } finally {
    spy.mockRestore();
  }
  expect(fs.existsSync(f.source)).toBe(true);
  expect(fs.readdirSync(f.target)).toEqual([]);

  // other errors are rethrown
  f = fixture();
  const failing = spyOn(fs, "renameSync").mockImplementation((): void => {
    throw new Error("EACCES: permission denied");
  });
  try {
    expect(
      await rejectionMessage(f.renamer.rename(await f.episode())),
    ).toContain("EACCES");
  } finally {
    failing.mockRestore();
  }
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

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
