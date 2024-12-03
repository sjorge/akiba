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
import type { LocalMapping } from "lib/anime/mapper/local";
import { readConfig } from "lib/config";
import { banner, log } from "lib/logger";

/*
 * Entrypoint `local-mapping` action for commander-js
 */
async function mappingAction(aid: number, opts: OptionValues): Promise<void> {
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
      mapping[aid].anilist = parseInt(`${opts.anilistid}`);
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
      mapping[aid].tmdb = parseInt(`${opts.tmdbid}`);
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
    fs.writeFileSync(mappingFile, toml.stringify(mapping), {
      encoding: "utf8",
    });
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
 * Setup `configure` command for commander-js
 */
export function addMappingCommand(program: Command): void {
  program
    .command("local-mapping")
    .description("manage local mapping entries")
    .addArgument(
      new Argument("<aid>", "anidb anime id to manage mapping for").argParser(
        (value: string) => {
          const id = parseInt(value, 10);
          if (isNaN(id)) throw new InvalidArgumentError("Expecting a number.");
          return id;
        },
      ),
    )
    .addOption(
      new Option("--anilistid <id>", "add maping to this anilist id").argParser(
        (value: string) => {
          const id = parseInt(value, 10);
          if (isNaN(id)) throw new InvalidArgumentError("Expecting a number.");
          return id;
        },
      ),
    )
    .option("--no-anilistid", "remove mapping to anilist id")
    .addOption(
      new Option(
        "--tmdbid <id>",
        "add maping to this themoviedb sereis id (movie ids are not supported)",
      ).argParser((value: string) => {
        const id = parseInt(value, 10);
        if (isNaN(id)) throw new InvalidArgumentError("Expecting a number.");
        return id;
      }),
    )
    .option("--no-tmdbid", "remove mapping to themoviedb series id")
    .addOption(new Option("--no-show", "show mapping for aid").default(true))
    .action(mappingAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
