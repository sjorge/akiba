import type { OptionValues } from "@commander-js/extra-typings";
import { Command, InvalidArgumentError } from "@commander-js/extra-typings";

/*
 * Entrypoint `configure` action for commander-js
 */
export async function configureAction(opts: OptionValues): Promise<void> {
  console.log("TODO: implement configure");
  console.log(opts);
}

/*
 * Setup `configure` command for commander-js
 */
export function addConfigureCommand(program: Command): void {
  program
    .command("configure")
    .description("update configuration file")
    .option("--anidb-client <client>", "your anidb http client name")
    .option(
      "--anidb-version <version>",
      "your anidb http client version",
      (value: string) => {
        const id = parseInt(value, 10);
        if (isNaN(id)) throw new InvalidArgumentError("Expecting a number.");
        return id;
      },
    )
    .option("--anidb-poster <yes/no>", "enable anidb poster fetching")
    .option("--anilist-token <token>", "your anilist http client token")
    .option("--tmdb-api-key <key>", "your tmdb API key")
    .option("--overwrite-nfo <yes/no>", "overwrite existing nfo by default")
    .action(configureAction);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
