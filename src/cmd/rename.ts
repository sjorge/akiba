import type { OptionValues } from "@commander-js/extra-typings";
import {
  Command,
  Option,
  InvalidArgumentError,
} from "@commander-js/extra-typings";
import fs from "node:fs";
import path from "node:path";

import type { Config } from "lib/config";

import { readConfig, validateConfig } from "lib/config";
import { banner, log } from "lib/logger";
import {
  AnimeRenamer,
  animeFormatValidate,
  AnimeFormatStringException,
} from "lib/anime/renamer";

/*
 * Entrypoint `rename` action for commander-js
 */
export async function renameAction(
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

  // initialize renamer
  banner();
  const animeRenamer: AnimeRenamer = new AnimeRenamer(
    config,
    opts.rehash as boolean,
    opts.format as string,
  );

  // identify episode file(s)
  // XXX: handle dir or single file
  if (!fs.existsSync(animePath) || !fs.statSync(animePath).isDirectory()) {
    log(`Directory "${animePath}" does not exist!`, "error");
    process.exitCode = 1;
    return;
  }
}

/*
 * Setup `rename` command for commander-js
 */
export function addRenameCommand(program: Command): void {
  program
    .command("rename")
    .description("rename episodes")
    .argument("<path>", "path to anime show directory or episode file")
    .addOption(
      new Option("--rehash", "force rehash of already hashed").default(false),
    )
    .addOption(
      new Option(
        "--overwrite",
        "overwrite if destination already exists",
      ).default(false),
    )
    .addOption(
      new Option(
        "--symlink",
        "create symlink from old filepath -> new filepath",
      ).default(false),
    )
    .addOption(
      new Option(
        "--format <format>",
        "overwrite format for the desination filename",
      ).argParser((value: string) => {
        try {
          animeFormatValidate(value, true);
        } catch (_e: unknown) {
          const e = _e as AnimeFormatStringException;
          throw new InvalidArgumentError(`${e.message}`);
        }
        return value;
      }),
    )
    .action(renameAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
