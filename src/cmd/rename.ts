import type { OptionValues } from "@commander-js/extra-typings";
import {
  Command,
  Argument,
  Option,
  InvalidArgumentError,
} from "@commander-js/extra-typings";
import fs from "node:fs";
import path from "node:path";
import naturalCompare from "natural-compare";
import { AnidbError } from "anidb-udp-client";

import type { Config } from "lib/config";
import type { AniDBMylistState, AnimeRenamerEpisode } from "lib/anime/renamer";

import { readConfig, validateConfig } from "lib/config";
import { banner, log } from "lib/logger";
import { ANIME_EPISODE_EXTENSIONS } from "lib/anime";
import {
  animeStringValidate,
  AnimeStringFormatException,
} from "lib/anime/formatter";
import { AnimeRenamer } from "lib/anime/renamer";

const IGNORE_EXTENSIONS: string[] = [".nfo", ".jpg", ".png"];
const IGNORE_FILES: string[] = [".plexmatch", ".ignore"];

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

  // parse cli flags
  const hashOnly = opts.printEd2klinks as boolean;
  const fresh = opts.fresh as boolean;
  const force = opts.force as boolean;
  const copy = opts.copy as boolean;
  const symlink = opts.symlink as boolean;
  const mylistState = opts.mylistState;
  const dryRun = opts.dryRun as boolean;

  // initialize renamer
  if (!hashOnly) banner();
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
        if (IGNORE_FILES.includes(episodeFile)) continue;
        if (IGNORE_EXTENSIONS.includes(path.extname(episodeFile))) continue;

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

  if (!hashOnly) {
    log(
      `Target Path: ${
        opts.targetPath
          ? path.resolve(`${opts.targetPath}`)
          : config.renamer.target_path || path.resolve(".")
      }`,
    );
    log(`Format: ${opts.format ? `${opts.format}` : config.renamer.format}`);
    if (episodeFiles.length > 0) {
      log("Renaming files ...");
    } else {
      log("Nothing to rename ...", "done");
    }
  }

  for (const episodeFile of episodeFiles.sort(naturalCompare)) {
    if (!hashOnly)
      log(`${path.basename(episodeFile)}: Identifying ...`, "step");

    // XXX: p-throttle to limit to 1x every 4 sec ?
    let episode: AnimeRenamerEpisode;

    try {
      episode = await animeRenamer.identify(episodeFile, fresh, hashOnly);
    } catch (_e: unknown) {
      const e = _e as AnidbError;
      log(`${path.basename(episodeFile)}: Identifying ...`, "error");
      log(`AniDB UDP Client: ${e.message}, try again in 30 mintues.`, "error");
      process.exitCode = 1;
      return;
    }

    if (hashOnly) {
      console.log(episode.ed2khash.link); // use console.log for raw output
    } else if (episode.data) {
      log(`${path.basename(episodeFile)}: Identifying ...`, "done");

      if (mylistState !== undefined && !dryRun) {
        log(
          `${path.basename(episodeFile)}: Updating mylist state to ${mylistState} ...`,
          "step",
        );
        try {
          log(
            `${path.basename(episodeFile)}: Updating mylist state to ${mylistState} ...`,
            (await animeRenamer.setMylistState(
              episode,
              mylistState as AniDBMylistState,
            ))
              ? "done"
              : "error",
          );
        } catch (_e: unknown) {
          const e = _e as AnidbError;
          log(
            `${path.basename(episodeFile)}: Updating mylist state to ${mylistState} ...`,
            "error",
          );
          log(
            `AniDB UDP Client: ${e.message}, try again in 30 mintues.`,
            "error",
          );
          process.exitCode = 1;
          return;
        }
      }

      log(`${path.basename(episodeFile)}: Renaming ...`, "step");
      const renamerResult = await animeRenamer.rename(
        episode,
        force,
        copy,
        symlink,
        dryRun,
      );
      if (renamerResult.destination_path !== undefined) {
        let message = "";
        if (renamerResult.action == "uptodate") {
          message = "Already correctly named.";
        } else {
          const renamerAction =
            renamerResult.action == "copy" ? "Copied" : "Moved";
          const renamerSymlinked =
            renamerResult.action == "symlink" ? " and symlinked" : "";
          message = `${renamerAction} to ${renamerResult.destination_path}${renamerSymlinked}!`;
        }

        log(`${path.basename(episodeFile)}: ${message}`, "done");
      } else {
        log(`${path.basename(episodeFile)}: Renaming ...`, "error");
      }
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
      )
        .default(false)
        .conflicts("copy"),
    )
    .addOption(
      new Option(
        "--mylist-state <state>",
        "update mylist state for identified files",
      ).choices(["internal_storage", "external_storage", "remote_storage"]),
    )
    .addOption(
      new Option("--dry-run", "do not rename the episodes").default(false),
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
      new Option("--print-ed2klinks", "only print ed2klinks")
        .default(false)
        .conflicts([
          "force",
          "copy",
          "symlink",
          "mylistState",
          "format",
          "targetPath",
          "dryRun",
        ]),
    )
    .action(renameAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
