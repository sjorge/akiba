import fs from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";
import { afterEach, beforeEach, expect, spyOn, test } from "bun:test";
import type { Mock } from "bun:test";
import { Command } from "@commander-js/extra-typings";
import AniDB from "anidbjs";
import axios from "axios";
import type { AxiosResponse } from "axios";

import * as logger from "lib/logger";
import { addNfoCommand } from "cmd/write-nfo";
import { writeConfig } from "lib/config";
import type { DeepPartial } from "test/helpers";
import {
  makeConfig,
  makeTempDir,
  rejectionMessage,
  seedCacheFile,
} from "test/helpers";
import { makeShow } from "test/fixtures/anidb-show";

type Logged = { type: string; msg: string };

let anime: Mock<(...args: unknown[]) => unknown>;
let get: Mock<(...args: unknown[]) => unknown>;

// NOTE: no test may reach the network, AniDB metadata is only asked for when
//       the show is not cached, and axios serves the title and list mappings
//       and the poster downloads
beforeEach((): void => {
  anime = spyOn(AniDB.prototype, "anime").mockRejectedValue(
    new Error("AniDB is not reachable in tests"),
  ) as Mock<(...args: unknown[]) => unknown>;
  get = spyOn(axios, "get").mockRejectedValue(
    new Error("the network is not reachable in tests"),
  ) as Mock<(...args: unknown[]) => unknown>;
});

afterEach((): void => {
  anime.mockRestore();
  get.mockRestore();
});

/*
 * A show directory with one episode file, the config a run needs (an HTTP
 * client, no poster unless asked for) and empty title and list mappings. The
 * AniDB show is served from its cache when `show` is given.
 */
function setUpRun(
  overrides: DeepPartial<ReturnType<typeof makeConfig>> = {},
  show?: object,
): string {
  // NOTE: makeConfig starts from the config file on disk, so every key the
  //       tests vary is set here, not left to the previous test
  const config = makeConfig({
    ...overrides,
    overwrite_nfo: overrides.overwrite_nfo ?? false,
    anidb: {
      http_client: { name: "akiba", version: 1 },
      poster: false,
      ...overrides.anidb,
    },
  });
  expect(writeConfig(config)).toBe(true);
  seedCacheFile(config.cache.path, "titles.map.toml", {});
  seedCacheFile(config.cache.path, "list.map.toml", {});
  if (show !== undefined) {
    seedCacheFile(config.cache.path, path.join("anidb", "100.aid.toml"), show);
  }

  const dir = makeTempDir();
  fs.writeFileSync(path.join(dir, "Test Show - 1 - Pilot.mkv"), "");
  return dir;
}

/*
 * Run `write-nfo` through commander. Returns what was logged and the exit
 * code the run left behind. The exit code is then reset, so the test runner
 * keeps its own.
 */
async function runWriteNfo(
  args: string[],
): Promise<{ logged: Logged[]; exitCode: number | undefined }> {
  const program = new Command()
    .exitOverride()
    .configureOutput({ writeOut: () => {}, writeErr: () => {} });
  addNfoCommand(program);

  const logged: Logged[] = [];
  const log = spyOn(logger, "log").mockImplementation(
    (msg: string, type: string = "info"): void => {
      logged.push({ type, msg });
    },
  );
  const banner = spyOn(logger, "banner").mockImplementation((): void => {});
  const previousExitCode = process.exitCode;
  let exitCode: number | undefined;
  try {
    process.exitCode = 0;
    await program.parseAsync(["write-nfo", ...args], { from: "user" });
    exitCode = process.exitCode;
  } finally {
    // NOTE: Bun ignores an undefined exit code, so 0 is the reset value
    process.exitCode = previousExitCode ?? 0;
    log.mockRestore();
    banner.mockRestore();
  }
  return { logged, exitCode };
}

const EPISODE_NFO_NAME = "Test Show - 1 - Pilot.nfo";
const POSTER_URL = "https://cdn.anidb.net/images/main/12345.jpg";

// what axios resolves with for a streamed poster download of `body`
function posterDownload(body: string): AxiosResponse {
  return {
    data: Readable.from([Buffer.from(body)]),
  } as unknown as AxiosResponse;
}

function requestedPoster(): boolean {
  return get.mock.calls.some(([url]) => url === POSTER_URL);
}

test("write-nfo keeps existing NFO files unless --force is given", async (): Promise<void> => {
  const dir = setUpRun({}, makeShow());
  fs.writeFileSync(path.join(dir, "tvshow.nfo"), "old");
  fs.writeFileSync(path.join(dir, EPISODE_NFO_NAME), "old");

  const kept = await runWriteNfo([dir, "--aid", "100"]);
  expect(kept.exitCode).toBe(0);
  expect(fs.readFileSync(path.join(dir, "tvshow.nfo"), "utf8")).toBe("old");
  expect(fs.readFileSync(path.join(dir, EPISODE_NFO_NAME), "utf8")).toBe("old");

  const forced = await runWriteNfo([dir, "--aid", "100", "--force"]);
  expect(forced.exitCode).toBe(0);
  expect(fs.readFileSync(path.join(dir, "tvshow.nfo"), "utf8")).toContain(
    "<tvshow>",
  );
  expect(fs.readFileSync(path.join(dir, EPISODE_NFO_NAME), "utf8")).toContain(
    "<episodedetails>",
  );
});

test("write-nfo labels a prefixed episode range with its written ends", async (): Promise<void> => {
  const dir = setUpRun({}, makeShow());
  fs.writeFileSync(path.join(dir, "Test Show - S1-S3 - Specials.mkv"), "");

  const { logged } = await runWriteNfo([dir, "--aid", "100"]);
  const messages = logged.map((l) => l.msg);
  expect(
    messages.some((m) => m.endsWith(": Writing episode S1-S3 NFO ...")),
  ).toBe(true);
  expect(messages.some((m) => m.includes("NaN"))).toBe(false);
});

test("write-nfo overwrites existing NFO files when overwrite_nfo is set", async (): Promise<void> => {
  const dir = setUpRun({ overwrite_nfo: true }, makeShow());
  fs.writeFileSync(path.join(dir, "tvshow.nfo"), "old");
  fs.writeFileSync(path.join(dir, EPISODE_NFO_NAME), "old");

  const { exitCode } = await runWriteNfo([dir, "--aid", "100"]);
  expect(exitCode).toBe(0);
  expect(fs.readFileSync(path.join(dir, "tvshow.nfo"), "utf8")).toContain(
    "<tvshow>",
  );
  expect(fs.readFileSync(path.join(dir, EPISODE_NFO_NAME), "utf8")).toContain(
    "<episodedetails>",
  );
});

test("write-nfo logs AniDB metadata errors and exits 1 instead of throwing", async (): Promise<void> => {
  const cases: [string, string, string][] = [
    [
      "302",
      "Please register a HTTP client on anidb and run 'configure' command again!",
      "302",
    ],
    ["500", "Please try again in 24h, we are currently banned!", "500"],
    ["998", "Anime with ID 100 is missing or invalid!", "998"],
    ["320", "Could not retrieve metadata!", "320"],
  ];
  for (const [, message, code] of cases) {
    const dir = setUpRun({});
    anime.mockImplementation(async (): Promise<never> => {
      throw Object.assign(new Error("reply"), { code });
    });

    const { logged, exitCode } = await runWriteNfo([dir, "--aid", "100"]);
    expect(exitCode, code).toBe(1);
    expect(
      logged.some((l) => l.type === "error" && l.msg.includes(message)),
      code,
    ).toBe(true);
    expect(fs.existsSync(path.join(dir, "tvshow.nfo")), code).toBe(false);
  }
});

test("write-nfo logs a show without titles and exits 1 instead of throwing", async (): Promise<void> => {
  const dir = setUpRun({}, makeShow({ titles: [] }));

  const { logged, exitCode } = await runWriteNfo([dir, "--aid", "100"]);
  expect(exitCode).toBe(1);
  // NOTE: the failure and its reason are logged on separate lines
  expect(
    logged.some(
      (l) => l.type === "error" && l.msg.includes("Failed to write show NFO"),
    ),
  ).toBe(true);
  expect(
    logged.some(
      (l) =>
        l.type === "error" &&
        l.msg.includes("AniDB anime 100 has no titles, refusing"),
    ),
  ).toBe(true);
  expect(fs.existsSync(path.join(dir, "tvshow.nfo"))).toBe(false);
});

test("write-nfo only warns when the poster download fails", async (): Promise<void> => {
  const dir = setUpRun({ anidb: { poster: true } }, makeShow());

  const { logged, exitCode } = await runWriteNfo([dir, "--aid", "100"]);
  expect(exitCode).toBe(0);
  expect(get).toHaveBeenCalled();
  expect(
    logged.some((l) => l.type === "warn" && l.msg.includes("poster")),
  ).toBe(true);
  expect(fs.existsSync(path.join(dir, "tvshow.nfo"))).toBe(true);
  expect(fs.existsSync(path.join(dir, EPISODE_NFO_NAME))).toBe(true);
  expect(fs.existsSync(path.join(dir, "poster.jpg"))).toBe(false);
});

test("write-nfo replaces an existing poster only with --force", async (): Promise<void> => {
  const dir = setUpRun({ anidb: { poster: true } }, makeShow());
  const poster = path.join(dir, "poster.jpg");
  fs.writeFileSync(poster, "OLD");
  get.mockImplementation(async (): Promise<AxiosResponse> =>
    posterDownload("NEW"),
  );

  const kept = await runWriteNfo([dir, "--aid", "100"]);
  expect(kept.exitCode).toBe(0);
  expect(requestedPoster()).toBe(false);
  expect(fs.readFileSync(poster, "utf8")).toBe("OLD");

  const forced = await runWriteNfo([dir, "--aid", "100", "--force"]);
  expect(forced.exitCode).toBe(0);
  expect(requestedPoster()).toBe(true);
  expect(fs.readFileSync(poster, "utf8")).toBe("NEW");
  expect(fs.readdirSync(dir).filter((name) => name.endsWith(".tmp"))).toEqual(
    [],
  );
});

test("write-nfo replaces an existing poster when overwrite_nfo is set", async (): Promise<void> => {
  const dir = setUpRun(
    { overwrite_nfo: true, anidb: { poster: true } },
    makeShow(),
  );
  const poster = path.join(dir, "poster.jpg");
  fs.writeFileSync(poster, "OLD");
  get.mockImplementation(async (): Promise<AxiosResponse> =>
    posterDownload("NEW"),
  );

  const { exitCode } = await runWriteNfo([dir, "--aid", "100"]);
  expect(exitCode).toBe(0);
  expect(requestedPoster()).toBe(true);
  expect(fs.readFileSync(poster, "utf8")).toBe("NEW");
});

test("write-nfo keeps the old poster and only warns when a forced download fails", async (): Promise<void> => {
  const dir = setUpRun({ anidb: { poster: true } }, makeShow());
  const poster = path.join(dir, "poster.jpg");
  fs.writeFileSync(poster, "OLD");

  const { logged, exitCode } = await runWriteNfo([
    dir,
    "--aid",
    "100",
    "--force",
  ]);
  expect(exitCode).toBe(0);
  expect(
    logged.some((l) => l.type === "warn" && l.msg.includes("poster")),
  ).toBe(true);
  expect(fs.readFileSync(poster, "utf8")).toBe("OLD");
  expect(fs.readdirSync(dir).filter((name) => name.endsWith(".tmp"))).toEqual(
    [],
  );
});

test("write-nfo never touches poster.jpg when poster fetching is off", async (): Promise<void> => {
  const dir = setUpRun({}, makeShow());
  const poster = path.join(dir, "poster.jpg");
  fs.writeFileSync(poster, "OLD");
  get.mockImplementation(async (): Promise<AxiosResponse> =>
    posterDownload("NEW"),
  );

  const { exitCode } = await runWriteNfo([dir, "--aid", "100", "--force"]);
  expect(exitCode).toBe(0);
  expect(requestedPoster()).toBe(false);
  expect(fs.readFileSync(poster, "utf8")).toBe("OLD");
});

test("write-nfo rejects id arguments that are not whole integers", async (): Promise<void> => {
  for (const args of [
    ["--aid", "12x"],
    ["--aid", "1e3"],
    ["--aid", "0x10"],
    ["--aid", "1.5"],
    ["--anilistid", "12x"],
    ["--tmdbid", "1e3"],
    ["--tmdbid", "0x10"],
  ]) {
    expect(
      await rejectionMessage(runWriteNfo(["x", ...args])),
      args.join(" "),
    ).toContain("Expecting a number.");
  }
});

test("write-nfo accepts padded whole integers and a negative tmdb marker", async (): Promise<void> => {
  const mapped = setUpRun({}, makeShow());
  const { exitCode } = await runWriteNfo([
    mapped,
    "--aid",
    " 100 ",
    "--tmdbid",
    " 56 ",
  ]);
  expect(exitCode).toBe(0);
  const nfo = fs.readFileSync(path.join(mapped, "tvshow.nfo"), "utf8");
  expect(nfo).toContain('type="tmdb"');
  expect(nfo).toContain(">56</uniqueid>");

  // NOTE: -1 marks the id as blacklisted, so no tmdb unique id is written
  const blacklisted = setUpRun({}, makeShow());
  await runWriteNfo([blacklisted, "--aid", "100", "--tmdbid", "-1"]);
  const blacklistedNfo = fs.readFileSync(
    path.join(blacklisted, "tvshow.nfo"),
    "utf8",
  );
  expect(blacklistedNfo).not.toContain('type="tmdb"');
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
