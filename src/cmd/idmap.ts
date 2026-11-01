import type { OptionValues } from "@commander-js/extra-typings";
import {
  Command,
  Option,
  Argument,
  InvalidArgumentError,
} from "@commander-js/extra-typings";
import fs from "node:fs";
import path from "node:path";
import toml from "@iarna/toml";
import { deepmergeInto } from "deepmerge-ts";

import type { Config } from "lib/config";
import type { LocalMapping } from "lib/anime/idmap/local";
import { readConfig } from "lib/config";
import { writeFileAtomic } from "lib/fs";
import { banner, log } from "lib/logger";
import { parseWholeInteger } from "lib/number";

// the stored id that records "no entry there, do not look it up again"
const BLACKLIST_ID = -1;

/*
 * Entrypoint `idmap` action for commander-js
 */
async function idmapAction(aid: number, opts: OptionValues): Promise<void> {
  const config: Config = readConfig();
  const mapping: LocalMapping = {};
  const mappingFile = path.join(config.cache.path, "local.map.toml");

  banner();
  log(`Using cache path: ${config.cache.path}`);
  if (
    !fs.existsSync(config.cache.path) ||
    !fs.statSync(config.cache.path).isDirectory()
  ) {
    try {
      fs.mkdirSync(config.cache.path, { recursive: true, mode: 0o750 });
    } catch {
      log("Could not create cache path!", "error");
      process.exitCode = 1;
      return;
    }
  }

  // read local mapping
  try {
    if (fs.existsSync(mappingFile)) {
      deepmergeInto(
        mapping,
        toml.parse(fs.readFileSync(mappingFile, "utf8")) as LocalMapping,
      );
    }
  } catch {
    log("Could not read local mappping file!", "error");
    log("Please check permissions, problem persists delete the mapping file.");
    log(`Mapping File: ${mappingFile}`);
    process.exitCode = 1;
    return;
  }

  // update local mapping
  if (opts.anilistid !== undefined) {
    if (opts.anilistid === false) {
      if (mapping[aid]?.anilist !== undefined) {
        log(`Unmapping AniList ID from AniDB ID ${aid}`);
        mapping[aid].anilist = undefined;
      }
    } else {
      if (mapping[aid] === undefined) mapping[aid] = {};
      mapping[aid].anilist = opts.anilistid as number;
    }
  }
  if (opts.tmdbid !== undefined) {
    if (opts.tmdbid === false) {
      if (mapping[aid]?.tmdb !== undefined) {
        log(`Unmapping tmdbid from AniDB ID ${aid}`);
        mapping[aid].tmdb = undefined;
        mapping[aid].tmdbSeason = undefined;
      }
    } else {
      if (mapping[aid] === undefined) mapping[aid] = {};
      mapping[aid].tmdb = opts.tmdbid as number;
      mapping[aid].tmdbSeason = 1;
    }
  }
  if (
    mapping[aid] &&
    mapping[aid].anilist === undefined &&
    mapping[aid].tmdb === undefined
  ) {
    delete mapping[aid];
  }

  // write local mapping
  try {
    writeFileAtomic(mappingFile, toml.stringify(mapping), 0o660);
    fs.chmodSync(mappingFile, 0o660);
  } catch {
    log("Could not write local mappping file!", "error");
    process.exitCode = 1;
    return;
  }

  // display mapping if --show
  if (opts.show) {
    if (mapping[aid]) {
      log(
        `AniDB ${aid} => AniList ${mapping[aid]?.anilist ? mapping[aid]?.anilist : "(no mapping)"}, TheMovieDB ${mapping[aid]?.tmdb ? `${mapping[aid]?.tmdb}:${mapping[aid]?.tmdbSeason}` : "(no mapping)"}`,
      );
    } else {
      log(`AniDB ${aid} has no mappings.`);
    }
  }
}

/*
 * Argument parser for an anidb id
 */
function parseId(value: string): number {
  const id = parseWholeInteger(value);
  if (id === undefined) throw new InvalidArgumentError("Expecting a number.");
  return id;
}

/*
 * Argument parser for a provider id, which may also be blacklisted with
 * `blacklist` or -1. Other negative numbers are rejected.
 */
function parseProviderId(value: string): number {
  if (value === "blacklist") return BLACKLIST_ID;
  const id = parseWholeInteger(value);
  if (id === undefined || id < BLACKLIST_ID) {
    throw new InvalidArgumentError("Expecting a number.");
  }
  return id;
}

/*
 * Setup `idmap` command for commander-js
 */
export function addIdmapCommand(program: Command): void {
  program
    .command("idmap")
    .description("manage local ID mapping entries")
    .addArgument(
      new Argument("<aid>", "anidb id to manage mapping for").argParser(
        parseId,
      ),
    )
    .addOption(
      new Option(
        "--anilistid <id>",
        "add mapping to this anilist id, or `blacklist` (-1) to record that this anime has no anilist entry, so it is not looked up again",
      ).argParser(parseProviderId),
    )
    .option(
      "--no-anilistid",
      "remove the anilist mapping, so it is looked up again (unlike blacklisting)",
    )
    .addOption(
      new Option(
        "--tmdbid <id>",
        "add mapping to this themoviedb series id (movie ids are not supported), or `blacklist` (-1) to record that this anime has no themoviedb series, so it is not looked up again",
      ).argParser(parseProviderId),
    )
    .option(
      "--no-tmdbid",
      "remove the themoviedb mapping, so it is looked up again (unlike blacklisting)",
    )
    .addOption(new Option("--no-show", "show mapping for aid").default(true))
    .action(idmapAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
