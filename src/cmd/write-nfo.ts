import type { OptionValues } from "@commander-js/extra-typings";
import {
  Command,
  Argument,
  Option,
  InvalidArgumentError,
} from "@commander-js/extra-typings";
import fs from "node:fs";
import path from "node:path";
import type { AniDB_Show } from "anidbjs";

import type { Config } from "lib/config";
import type { AnimeId, AnimeTitleVariant } from "lib/anime";

import { readConfig, validateConfig } from "lib/config";
import { AnimeIdmapLocal } from "lib/anime/idmap/local";
import { AnimeIdmapList } from "lib/anime/idmap/list";
import { AnimeIdmapAnilist } from "lib/anime/idmap/anilist";
import { AnimeIdmapTmdb } from "lib/anime/idmap/tmdb";
import { AnimeResolver } from "lib/anime/resolver";
import { AnimeMetadata } from "lib/anime/metadata";
import { AnimeShowNfo, AnimeEpisodeNfo } from "lib/anime/nfo";
import { banner, log } from "lib/logger";
import { parseWholeInteger } from "lib/number";

function handleError(msg: string, _e: unknown): void {
  const e = _e as Error;
  log(`${msg}: ${e.message}`, "error");
  process.exitCode = 1;
}

/*
 * Commander argument parser for ids, the whole value must be an integer
 */
function parseIdArgument(value: string): number {
  const id = parseWholeInteger(value);
  if (id === undefined) throw new InvalidArgumentError("Expecting a number.");
  return id;
}

/*
 * Entrypoint `write-nfo` action for commander-js
 */
export async function nfoAction(
  animePath: string,
  opts: OptionValues,
): Promise<void> {
  // read config and handle some overrides
  const config: Config = readConfig();
  if (!validateConfig(config, "write-nfo", true)) {
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
  } catch (_e: unknown) {
    handleError("Failed to initialize anime resolver", _e);
    return;
  }

  try {
    animeIdmapLocal = new AnimeIdmapLocal(config);
    await animeIdmapLocal.refresh();
  } catch (_e: unknown) {
    handleError("Failed to initialize anime local mapper", _e);
    return;
  }

  try {
    animeIdmapList = new AnimeIdmapList(config);
    await animeIdmapList.refresh();
  } catch (_e: unknown) {
    handleError("Failed to initialize anime list mapper", _e);
    return;
  }

  try {
    if (config.anilist.token)
      animeIdmapAnilist = new AnimeIdmapAnilist(config, animeResolver);
    if (config.tmdb.api_key)
      animeIdmapTmdb = new AnimeIdmapTmdb(config, animeResolver);
  } catch (_e: unknown) {
    handleError("Failed to initialize anime remote mappers", _e);
    return;
  }

  try {
    animeMetadata = new AnimeMetadata(config);
  } catch (_e: unknown) {
    handleError("Failed to initialize anime metadata retriever", _e);
    return;
  }

  log(`${title}: Identifying ...`, "step", true, id);
  // NOTE: the id options are already whole integers, see parseIdArgument
  id = opts.aid
    ? ({ anidb: opts.aid as number } as AnimeId)
    : animeResolver.id(title);

  // unable to identify anime, exit
  if (id?.anidb === undefined) {
    log(`${title}: Failed to identify anime!`, "error", true, id);
    process.exitCode = 1;
    return;
  }

  // normalize title
  animeResolver.title(id)?.forEach((t: AnimeTitleVariant) => {
    if (t.type == "main" && t.language == "x-jat") {
      title = t.title;
    }
  });
  log(`${title}: Identifying ...`, "done", true, id);

  // add additional ids from cli flags
  log(`${title}: Mapping IDs from cli flags ...`, "step", true, id);
  if (opts.anilistid) {
    id.anilist = opts.anilistid as number;
  }
  if (opts.tmdbid) {
    id.tmdb = opts.tmdbid as number;
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
  log(`${title}: Retrieving metadata ...`, "step", true, id);
  let metadata: AniDB_Show;
  try {
    metadata = await animeMetadata.get(id, opts.fresh as boolean);
  } catch (_e: unknown) {
    const e = _e as Error;
    log(`${title}: ${e.message}`, "error", true, id);
    process.exitCode = 1;
    return;
  }
  log(`${title}: Retrieving metadata ...`, "done", true, id);

  // NOTE: --force and the overwrite_nfo config key both replace existing NFOs
  const overwrite: boolean = (opts.force as boolean) || config.overwrite_nfo;

  // write tvshow.nfo
  log(`${title}: Writing show NFO ...`, "step", true, id);
  let animeShowNfo: AnimeShowNfo;
  try {
    animeShowNfo = new AnimeShowNfo(id, metadata, animePath);
  } catch (_e: unknown) {
    const e = _e as Error;
    log(`${title}: Failed to write show NFO ...`, "error", true, id);
    log(e.message, "error");
    process.exitCode = 1;
    return;
  }
  if (
    !animeShowNfo.isValid() ||
    !(await animeShowNfo.write(overwrite, false))
  ) {
    log(`${title}: Failed to write show NFO ...`, "error", true, id);
    process.exitCode = 1;
    return;
  }
  log(`${title}: Writing show NFO ...`, "done", true, id);

  // NOTE: the poster is best effort, a failed download only warns
  if (config.anidb.poster && !(await animeShowNfo.writePoster(overwrite))) {
    log(`${title}: Failed to write the poster ...`, "warn", true, id);
  }

  // write episode.nfo
  for (const episode of animeResolver.episodes(animePath)) {
    let titleSuffix: string = `${episode.episodeStart}`;
    if (episode.episodeEnd != episode.episodeStart)
      titleSuffix = `${titleSuffix}-${episode.episodeEnd}`;

    log(`${title}: Writing episode ${titleSuffix} NFO ...`, "step", true, id);
    const animeEpisodeNfo = new AnimeEpisodeNfo(id, episode, metadata);
    if (
      !animeEpisodeNfo.isValid() ||
      !(await animeEpisodeNfo.write(overwrite))
    ) {
      log(
        `${title}: Failed to write episode ${titleSuffix} NFO ...`,
        "error",
        true,
        id,
      );
      process.exitCode = 1;
    } else {
      log(`${title}: Writing episode ${titleSuffix} NFO ...`, "done", true, id);
    }
  }
}

/*
 * Setup `write-nfo` command for commander-js
 */
export function addNfoCommand(program: Command): void {
  program
    .command("write-nfo")
    .description("write NFO files")
    .addArgument(
      new Argument("<path>", "path to anime show directory").argParser(
        (value: string) => {
          if (!path.isAbsolute(value)) return path.resolve(value);
          return value;
        },
      ),
    )
    .addOption(
      new Option(
        "--aid <id>",
        "specify the anidb anime id instead of trying to auto detect",
      ).argParser(parseIdArgument),
    )
    .addOption(
      new Option(
        "--anilistid <id>",
        "specify the anilist id instead of trying to auto detect",
      ).argParser(parseIdArgument),
    )
    .addOption(
      new Option(
        "--tmdbid <id>",
        "specify the themoviedb series id instead of trying to auto detect (movie ids are not supported)",
      ).argParser(parseIdArgument),
    )
    .addOption(new Option("--fresh", "force retrieve metadata").default(false))
    .addOption(
      new Option("--force", "overwrite existing NFO files").default(false),
    )
    .action(nfoAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
