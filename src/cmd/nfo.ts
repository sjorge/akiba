import type { OptionValues } from "@commander-js/extra-typings";
import {
  Command,
  Option,
  InvalidArgumentError,
} from "@commander-js/extra-typings";
import fs from "node:fs";
import path from "node:path";

import type { Config } from "lib/config";
import type { TvShow, Episode, UniqueId } from "lib/nfo";
import type { AnimeId } from "lib/anime";
import { readConfig, validateConfig } from "lib/config";
import { TvShowNfo, EpisodeNfo } from "lib/nfo";
import { banner, log } from "lib/logger";

/*
 * Entrypoint `write-nfo` action for commander-js
 */
export async function nfoAction(
  animePath: string,
  opts: OptionValues,
): Promise<void> {
  // read config and handle some overrides
  const config: Config = readConfig();
  if (!validateConfig(config, true)) {
    log(
      "Please run 'configure' and configure at least --anidb-client and --anidb-version!",
      "error",
    );
    process.exitCode = 1;
    return;
  }

  if (opts.force) config.overwrite_nfo = true;

  // identify anime
  banner();
  if (!fs.existsSync(animePath) || !fs.statSync(animePath).isDirectory()) {
    log(`Directory "${animePath}" does not exist!`, "error");
    process.exitCode = 1;
    return;
  }

  let title: string = path.basename(animePath);
  let id: AnimeId | undefined;

  log(`${title}: Identifying ...`, "step", true, id);
  if (opts.aid) id = { anidb: parseInt(`${opts.aid}`) } as AnimeId;
  // XXX: detect anidb from path
  if (id?.anidb) {
    if (opts.anilistid) {
      id.anilist = parseInt(`${opts.anilistid}`);
      log(`${title}: Identifying ...`, "step", true, id);
    }
    // XXX: try map from aid
    // XXX: try search anilist
    if (opts.tmdbid) {
      id.tmdb = parseInt(`${opts.tmdbid}`);
      id.tmdbSeason = 1; // hardcode this for now
      log(`${title}: Identifying ...`, "step", true, id);
    }
    // XXX: try map from aid
    // XXX: try search themoviedb
    log(`${title}: Identifying ...`, "done", true, id);
  } else {
    log(`${title}: Failed to identify anime!`, "error", true, id);
    process.exitCode = 1;
    return;
  }

  // XXX: write nfos
}

/*
 * Setup `write-nfo` command for commander-js
 */
export function addNfoCommand(program: Command): void {
  program
    .command("write-nfo")
    .description("write NFO files")
    .argument("<path>", "path to anime show directory")
    .addOption(
      new Option(
        "--aid <id>",
        "specify the anidb anime id instead of trying to auto detect",
      ).argParser((value: string) => {
        const id = parseInt(value, 10);
        if (isNaN(id)) throw new InvalidArgumentError("Expecting a number.");
        return id;
      }),
    )
    .addOption(
      new Option(
        "--anilistid <id>",
        "specify the anilist anime id instead of trying to auto detect",
      ).argParser((value: string) => {
        const id = parseInt(value, 10);
        if (isNaN(id)) throw new InvalidArgumentError("Expecting a number.");
        return id;
      }),
    )
    .addOption(
      new Option(
        "--tmdbid <id>",
        "specify the themoviedb series id instead of trying to auto detect (movie ids are not supported)",
      ).argParser((value: string) => {
        const id = parseInt(value, 10);
        if (isNaN(id)) throw new InvalidArgumentError("Expecting a number.");
        return id;
      }),
    )
    .addOption(new Option("--fresh", "force update metadata").default(false))
    .addOption(
      new Option("--force", "overwrite existing NFO files").default(false),
    )
    .action(nfoAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
