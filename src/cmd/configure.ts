import type { OptionValues } from "@commander-js/extra-typings";
import {
  Command,
  Option,
  InvalidArgumentError,
} from "@commander-js/extra-typings";
import type { Config } from "lib/config";
import {
  readConfig,
  writeConfig,
  validateConfig,
  configFile,
} from "lib/config";
import { banner, log } from "lib/logger";

/*
 * Entrypoint `configure` action for commander-js
 */
async function configureAction(opts: OptionValues): Promise<void> {
  const config: Config = readConfig();

  if (opts.anidbClient) config.anidb.client.name = `${opts.anidbClient}`;
  if (opts.anidbVersion)
    config.anidb.client.version = parseInt(`${opts.anidbVersion}`, 10);
  if (typeof opts.anidbPoster == "boolean")
    config.anidb.poster = opts.anidbPoster;
  if (opts.tmdbApiKey) config.tmdb.api_key = `${opts.tmdbApiKey}`;
  if (opts.anilistToken) config.anilist.token = `${opts.anilistToken}`;
  if (typeof opts.overwriteNfo == "boolean")
    config.overwrite_nfo = opts.overwriteNfo;

  if (!writeConfig(config)) {
    log(`Failed to update ${configFile}!`, "error");
    process.exitCode = 1;
  } else if (!validateConfig(config, true)) {
    process.exitCode = 1;
  }

  if (opts.dump) {
    banner();
    console.log(JSON.stringify(config, null, 2));
  }
}

/*
 * Setup `configure` command for commander-js
 */
export function addConfigureCommand(program: Command): void {
  program
    .command("configure")
    .description("update configuration file")
    .option("--anidb-client <client>", "your anidb http client name")
    .addOption(
      new Option(
        "--anidb-version <version>",
        "your anidb http client version",
      ).argParser((value: string) => {
        const id = parseInt(value, 10);
        if (isNaN(id)) throw new InvalidArgumentError("Expecting a number.");
        return id;
      }),
    )
    .option("--anidb-poster", "enable anidb poster fetching")
    .option("--no-anidb-poster", "disable anidb poster fetching")
    .option("--anilist-token <token>", "your anilist http client token")
    .option("--tmdb-api-key <key>", "your tmdb API key")
    .option("--overwrite-nfo", "overwrite existing nfo by default")
    .option("--no-overwrite-nfo", "keep existing nfo by default")
    .option("--dump", "dump configuration")
    .action(configureAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
