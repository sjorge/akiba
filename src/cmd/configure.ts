import fs from "node:fs";
import path from "node:path";
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
import {
  animeValidateString,
  AnimeFormatStringException,
} from "lib/anime/renamer";
import { banner, log } from "lib/logger";

/*
 * Entrypoint `configure` action for commander-js
 */
async function configureAction(opts: OptionValues): Promise<void> {
  const config: Config = readConfig();

  if (opts.anidbHttpClient) {
    const httpClientVersion = (opts.anidbHttpClient as string).split("/");
    if (httpClientVersion.length == 1) httpClientVersion.push("1");
    config.anidb.http_client.name = httpClientVersion[0];
    config.anidb.http_client.version = parseInt(httpClientVersion[1], 10);
  }

  if (opts.anidbUdpClient) {
    const udpClientVersion = (opts.anidbUdpClient as string).split("/");
    if (udpClientVersion.length == 1) udpClientVersion.push("1");
    config.anidb.udp_client.name = udpClientVersion[0];
    config.anidb.udp_client.version = parseInt(udpClientVersion[1], 10);
  }

  if (opts.anidbAuthUsername)
    config.anidb.udp_client.username = `${opts.anidbAuthUsername}`;
  if (opts.anidbAuthPassword)
    config.anidb.udp_client.password = `${opts.anidbAuthPassword}`;

  if (typeof opts.anidbPoster == "boolean")
    config.anidb.poster = opts.anidbPoster;

  if (opts.tmdbApiKey) config.tmdb.api_key = `${opts.tmdbApiKey}`;
  if (opts.anilistToken) config.anilist.token = `${opts.anilistToken}`;

  if (typeof opts.overwriteNfo == "boolean")
    config.overwrite_nfo = opts.overwriteNfo;

  if (opts.renameFormat) config.renamer.format = `${opts.renameFormat}`;
  if (typeof opts.renameTargetPath == "boolean") {
    config.renamer.targetPath = undefined;
  } else {
    config.renamer.targetPath = `${opts.renameTargetPath}`;
  }

  if (!writeConfig(config)) {
    log(`Failed to update ${configFile}!`, "error");
    process.exitCode = 1;
  } else if (!validateConfig(config, "write-nfo", true)) {
    process.exitCode = 1;
  } else if (!validateConfig(config, "renamer", true)) {
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
  function anidbClientValidation(value: string): string {
    const clientVersion = value.split("/");
    if (clientVersion.length == 1) clientVersion.push("1");

    if (isNaN(parseInt(clientVersion[1], 10)))
      throw new InvalidArgumentError("Expecting a number for version.");

    return value;
  }

  program
    .command("configure")
    .description("update configuration file")
    .addOption(
      new Option(
        "--anidb-http-client <http_client>",
        "your anidb http client (format: <name>/<version>)",
      ).argParser(anidbClientValidation),
    )
    .addOption(
      new Option(
        "--anidb-udp-client <udp_client>",
        "your anidb udp client (format: <name>/<version>)",
      ).argParser(anidbClientValidation),
    )
    .addOption(
      new Option("--anidb-auth-username <username>", "your anidb username"),
    )
    .addOption(
      new Option("--anidb-auth-password <password>", "your anidb password"),
    )
    .option("--anidb-poster", "enable anidb poster fetching")
    .option("--no-anidb-poster", "disable anidb poster fetching")
    .option("--anilist-token <token>", "your anilist http client token")
    .option("--tmdb-api-key <key>", "your tmdb API key")
    .option("--overwrite-nfo", "overwrite existing nfo by default")
    .option("--no-overwrite-nfo", "keep existing nfo by default")
    .addOption(
      new Option(
        "--rename-format <format>",
        "format for the desination filename",
      ).argParser((value: string) => {
        try {
          animeValidateString(value, true);
        } catch (_e: unknown) {
          const e = _e as AnimeFormatStringException;
          throw new InvalidArgumentError(`${e.message}`);
        }
        return value;
      }),
    )
    .addOption(
      new Option("--rename-target-path <path>", "desination path").argParser(
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
    .option("--no-rename-target-path", "use current working directory")
    .option("--dump", "dump configuration")
    .action(configureAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
