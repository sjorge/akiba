import type { OptionValues } from "@commander-js/extra-typings";
import {
  Command,
  Argument,
  Option,
  InvalidArgumentError,
} from "@commander-js/extra-typings";
import fs from "node:fs";
import path from "node:path";

import type { Config } from "lib/config";

import { readConfig, validateConfig } from "lib/config";
import { banner, log } from "lib/logger";
import { ANIME_EPISODE_EXTENSIONS } from "lib/anime";
import {
  animeStringValidate,
  AnimeStringFormatException,
} from "lib/anime/formatter";
import { AnimeRenamer } from "lib/anime/renamer";

const ignoreExt: string[] = [".nfo", ".jpg", ".png"];
const ignoreFile: string[] = [".plexmatch", ".ignore"];

/*
 * Entrypoint `rename` action for commander-js
 */
export async function renameAction(
  animePath: string,
  opts: OptionValues,
): Promise<void> {
  // read config and handle some overrides
  const config: Config = readConfig();
  if (!validateConfig(config, "renamer", true)) {
    process.exitCode = 1;
    return;
  }

  // initialize renamer
  banner();
  const animeRenamer: AnimeRenamer = new AnimeRenamer(
    config,
    opts.rehash as boolean,
    opts.format ? `${opts.format}` : undefined,
    opts.targetPath ? `${opts.targetPath}` : undefined,
  );

  // identify episode file(s)
  if (!fs.existsSync(animePath)) {
    log(`Directory or file "${animePath}" does not exist!`, "error");
    process.exitCode = 1;
    return;
  }

  const episodeFiles: string[] = [];
  if (fs.statSync(animePath).isDirectory()) {
    function _episodeWalkTree(dirPath: string, episodes: string[]): void {
      for (const episodeFile of fs.readdirSync(dirPath)) {
        const episodePath = path.join(dirPath, episodeFile);

        // ignore certain files and extension
        if (ignoreFile.includes(episodeFile)) continue;
        if (ignoreExt.includes(path.extname(episodeFile))) continue;

        // ignore symlinks
        const episodeFsStats = fs.lstatSync(episodePath);
        if (episodeFsStats.isSymbolicLink()) continue;

        // recurse if directory
        if (episodeFsStats.isDirectory()) {
          _episodeWalkTree(episodePath, episodes);
        }

        // ignore unwanted extensions
        if (!ANIME_EPISODE_EXTENSIONS.includes(path.extname(episodeFile)))
          continue;

        episodes.push(episodePath);
      }
    }

    _episodeWalkTree(animePath, episodeFiles);
  } else {
    episodeFiles.push(animePath);
  }

  const hashOnly = opts.printEd2klinks as boolean;
  const fresh = opts.fresh as boolean;
  const force = opts.force as boolean;
  const copy = opts.copy as boolean;
  const symlink = opts.symlink as boolean;

  if (hashOnly) {
    log("Printing ed2khash links ...");
  } else {
    log(
      `Target Path: ${
        opts.targetPath
          ? path.resolve(`${opts.targetPath}`)
          : config.renamer.target_path || path.resolve(".")
      }`,
    );
    log(`Format: ${opts.format ? `${opts.format}` : config.renamer.format}`);
    log("Renaming files ...");
  }

  for (const episodeFile of episodeFiles) {
    if (!hashOnly)
      log(`${path.basename(episodeFile)}: Identifying ...`, "step");

    // XXX: p-throttle to limit to 1x every 4 sec ?
    const episode = await animeRenamer.identify(episodeFile, fresh, hashOnly);

    if (hashOnly) {
      console.log(episode.ed2khash.link); // use console.log for raw output
    } else if (episode.data) {
      log(`${path.basename(episodeFile)}: Identifying ...`, "done");
      await animeRenamer.rename(episode, force, copy, symlink);
    } else {
      log(`${path.basename(episodeFile)}: Identifying ...`, "error");
      process.exitCode = 1;
    }
  }

  await animeRenamer.destroy();
}

/*
 * Setup `rename` command for commander-js
 */
export function addRenameCommand(program: Command): void {
  program
    .command("rename")
    .description("rename episodes")
    .addArgument(
      new Argument(
        "<path>",
        "path to anime show directory or episode file",
      ).argParser((value: string) => {
        if (!path.isAbsolute(value)) return path.resolve(value);
        return value;
      }),
    )
    .addOption(
      new Option("--rehash", "force rehash of already hashed").default(false),
    )
    .addOption(new Option("--fresh", "force retrieve metadata").default(false))
    .addOption(
      new Option("--force", "overwrite if destination exists").default(false),
    )
    .addOption(new Option("--copy", "copy file instead of move").default(false))
    .addOption(
      new Option(
        "--symlink",
        "create symlink at old source path to new destination path",
      ).default(false),
    )
    .addOption(
      new Option(
        "--format <format>",
        "overwrite format for the desination filename",
      ).argParser((value: string) => {
        try {
          animeStringValidate(value, true);
        } catch (_e: unknown) {
          const e = _e as AnimeStringFormatException;
          throw new InvalidArgumentError(`${e.message}`);
        }
        return value;
      }),
    )
    .addOption(
      new Option("--target-path <path>", "overwrite desination path").argParser(
        (value: string) => {
          if (!path.isAbsolute(value)) value = path.resolve(value);

          if (!fs.existsSync(value) || !fs.lstatSync(value).isDirectory()) {
            throw new InvalidArgumentError(
              `Path ${value} is no a directory or does not exist!`,
            );
          }

          return value;
        },
      ),
    )
    .addOption(
      new Option("--print-ed2klinks", "only print ed2klinks").default(false),
    )
    .action(renameAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
