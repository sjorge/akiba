import fs from "node:fs";
import path from "node:path";
import { expect, spyOn, test } from "bun:test";
import { Command } from "@commander-js/extra-typings";
import toml from "@iarna/toml";

import * as logger from "lib/logger";
import { addIdmapCommand } from "cmd/idmap";
import { writeConfig } from "lib/config";
import { makeConfig, rejectionMessage } from "test/helpers";

/*
 * Point the command at a fresh cache directory and return its mapping file
 */
function useFreshCache(): string {
  const config = makeConfig();
  expect(writeConfig(config)).toBe(true);
  return path.join(config.cache.path, "local.map.toml");
}

/*
 * Run `idmap` through commander and return the messages it logged
 */
async function runIdmap(args: string[]): Promise<string[]> {
  const program = new Command()
    .exitOverride()
    .configureOutput({ writeOut: () => {}, writeErr: () => {} });
  addIdmapCommand(program);

  const messages: string[] = [];
  const log = spyOn(logger, "log").mockImplementation((msg: string): void => {
    messages.push(msg);
  });
  const banner = spyOn(logger, "banner").mockImplementation((): void => {});
  const previousExitCode = process.exitCode;
  try {
    await program.parseAsync(["idmap", ...args], { from: "user" });
  } finally {
    process.exitCode = previousExitCode;
    log.mockRestore();
    banner.mockRestore();
  }
  return messages;
}

// NOTE: the stored mappings cover the four --show outputs: a missing
//       tmdbSeason, a missing or zero id, a blacklisted TMDB id and an
//       unknown aid
test("idmap --show prints the stored mapping, with its placeholders", async (): Promise<void> => {
  const mappingFile = useFreshCache();
  fs.writeFileSync(
    mappingFile,
    [
      "[1]",
      "anilist = 5",
      "tmdb = 7",
      "tmdbSeason = 2",
      "",
      "[2]",
      "tmdb = -1",
      "",
      "[3]",
      "anilist = 0",
      "tmdb = 0",
      "",
      "[5]",
      "tmdb = 7",
      "",
    ].join("\n"),
  );

  expect(await runIdmap(["1"])).toContain(
    "AniDB 1 => AniList 5, TheMovieDB 7:2",
  );
  expect(await runIdmap(["2"])).toContain(
    "AniDB 2 => AniList (no mapping), TheMovieDB -1:undefined",
  );
  expect(await runIdmap(["3"])).toContain(
    "AniDB 3 => AniList (no mapping), TheMovieDB (no mapping)",
  );
  expect(await runIdmap(["5"])).toContain(
    "AniDB 5 => AniList (no mapping), TheMovieDB 7:undefined",
  );
  expect(await runIdmap(["4"])).toContain("AniDB 4 has no mappings.");
});

test("idmap accepts -1 and blacklist for tmdbid and anilistid, stored as -1", async (): Promise<void> => {
  const mappingFile = useFreshCache();

  await runIdmap(["1", "--tmdbid", "-1", "--no-show"]);
  expect(toml.parse(fs.readFileSync(mappingFile, "utf8"))).toMatchObject({
    1: { tmdb: -1 },
  });

  await runIdmap(["2", "--tmdbid", "blacklist", "--no-show"]);
  await runIdmap(["2", "--anilistid", "blacklist", "--no-show"]);
  await runIdmap(["3", "--anilistid", "-1", "--no-show"]);
  expect(toml.parse(fs.readFileSync(mappingFile, "utf8"))).toMatchObject({
    2: { tmdb: -1, anilist: -1 },
    3: { anilist: -1 },
  });
});

test("idmap rejects other negative ids and anything that is not a whole integer", async (): Promise<void> => {
  useFreshCache();

  for (const args of [
    ["1", "--tmdbid", "-2"],
    ["1", "--anilistid", "-2"],
    ["1", "--tmdbid", "Blacklist"],
    ["1", "--tmdbid", "blacklisted"],
    ["1", "--tmdbid", "12x"],
    ["1", "--tmdbid", "1e3"],
    ["1", "--tmdbid", "0x10"],
    ["1", "--tmdbid", "1.5"],
    ["1", "--anilistid", "12x"],
    ["12x"],
    ["0x10"],
  ]) {
    expect(await rejectionMessage(runIdmap(args)), args.join(" ")).toContain(
      "Expecting a number.",
    );
  }
});

test("idmap accepts a padded whole integer", async (): Promise<void> => {
  const mappingFile = useFreshCache();

  await runIdmap(["7", "--tmdbid", " 259140 ", "--no-show"]);
  expect(toml.parse(fs.readFileSync(mappingFile, "utf8"))).toMatchObject({
    7: { tmdb: 259140, tmdbSeason: 1 },
  });
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
