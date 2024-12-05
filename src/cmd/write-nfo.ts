import type { OptionValues } from "@commander-js/extra-typings";
import {
  Command,
  Option,
  InvalidArgumentError,
} from "@commander-js/extra-typings";
import fs from "node:fs";
import path from "node:path";

import type { Config } from "lib/config";
import type { AnimeId, AnimeTitleVariant } from "lib/anime";

import { readConfig, validateConfig } from "lib/config";
import { AnimeIdmapLocal } from "lib/anime/idmap/local";
import { AnimeIdmapList } from "lib/anime/idmap/list";
import { AnimeIdmapAnilist } from "lib/anime/idmap/anilist";
import { AnimeIdmapTmdb } from "lib/anime/idmap/tmdb";
import { AnimeResolver } from "lib/anime/resolver";
import { AnimeMetadata } from "lib/anime/metadata";
import { AnimeShowNfo } from "lib/anime/nfo";
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
  let id: AnimeId | undefined = undefined;

  let animeResolver: AnimeResolver;
  let animeIdmapLocal: AnimeIdmapLocal;
  let animeIdmapList: AnimeIdmapList;
  let animeIdmapAnilist: AnimeIdmapAnilist | undefined;
  let animeIdmapTmdb: AnimeIdmapTmdb | undefined;
  let animeMetadata: AnimeMetadata;

  try {
    animeResolver = new AnimeResolver(config);
    await animeResolver.refresh();

    animeIdmapLocal = new AnimeIdmapLocal(config);
    await animeIdmapLocal.refresh();

    animeIdmapList = new AnimeIdmapList(config);
    await animeIdmapList.refresh();

    if (config.anilist.token)
      animeIdmapAnilist = new AnimeIdmapAnilist(config, animeResolver);
    if (config.tmdb.api_key)
      animeIdmapTmdb = new AnimeIdmapTmdb(config, animeResolver);

    animeMetadata = new AnimeMetadata(config);
  } catch (_e: unknown) {
    const e = _e as Error;
    log(`Failed to initialize anime mappers!`, "error");
    log(e.message, "error");
    process.exitCode = 1;
    return;
  }

  log(`${title}: Identifying ...`, "step", true, id);
  id = opts.aid
    ? ({ anidb: parseInt(`${opts.aid}`) } as AnimeId)
    : animeResolver.resolveFromTitle(title);

  // unable to identify anime, exit
  if (id?.anidb === undefined) {
    log(`${title}: Failed to identify anime!`, "error", true, id);
    process.exitCode = 1;
    return;
  }

  // normalize title
  animeResolver.titleFromId(id)?.forEach((t: AnimeTitleVariant) => {
    if (t.type == "main" && t.language == "x-jat") {
      title = t.title;
    }
  });
  log(`${title}: Identifying ...`, "done", true, id);

  // add additional ids from cli flags
  log(`${title}: Mapping IDs from cli flags ...`, "step", true, id);
  if (opts.anilistid) {
    id.anilist = parseInt(`${opts.anilistid}`);
  }
  if (opts.tmdbid) {
    id.tmdb = parseInt(`${opts.tmdbid}`);
    id.tmdbSeason = 1; // hardcode this for now
  }
  log(`${title}: Mapping IDs from cli flags ...`, "done", true, id);

  // add additional ids from local and online lists
  log(`${title}: Mapping IDs from local mapping ...`, "step", true, id);
  animeIdmapLocal.apply(id);
  log(`${title}: Mapping IDs from local mapping ...`, "done", true, id);

  log(`${title}: Mapping IDs from list mapping ...`, "step", true, id);
  animeIdmapList.apply(id);
  log(`${title}: Mapping IDs from list mapping ...`, "done", true, id);

  // (optionally) search providers for additional ids
  if (animeIdmapAnilist && id.anilist === undefined) {
    log(`${title}: Looking up Anilist ID ...`, "step", true, id);
    await animeIdmapAnilist.lookup(title, id);
    log(`${title}: Looking up Anilist ID ...`, "done", true, id);
  }

  if (animeIdmapTmdb && id.tmdb === undefined) {
    log(`${title}: Looking up Tmdb ID ...`, "step", true, id);
    await animeIdmapTmdb.apply(id);
    log(`${title}: Looking up Tmdb ID ...`, "done", true, id);
  }

  // lookup metadata
  log(`${title}: Retrieving metadata ...`, "step");
  const metadata = await animeMetadata.get(id, opts.fresh as boolean);
  log(`${title}: Retrieving metadata ...`, "done");

  // write nfo files
  log(`${title}: Writing tshow.nfo ...`, "step");
  const animeShowNfo = new AnimeShowNfo(id, metadata, animePath);
  await animeShowNfo.write(opts.force as boolean, config.anidb.poster);
  log(`${title}: Writing tshow.nfo ...`, "done");
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
        "specify the anilist id instead of trying to auto detect",
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
    .addOption(
      new Option("--fresh", "force retreiving metadata").default(false),
    )
    .addOption(
      new Option("--force", "overwrite existing NFO files").default(false),
    )
    .action(nfoAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab