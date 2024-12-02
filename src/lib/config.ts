import { _DEFINE_PROG } from "vars";
import os from "node:os";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import toml from "@iarna/toml";
import { deepmerge } from "deepmerge-ts";
import { banner, log } from "lib/logger";

export type Config = {
  anidb: {
    url: string;
    client: {
      name?: string;
      version?: number;
    };
    poster: boolean;
  };
  anilist: {
    token?: string;
  };
  tmdb: {
    api_key?: string;
  };
  cache: {
    path: string;
    anidb_age: number;
    mapping_age: number;
  };
  overwrite_nfo: boolean;
};

const configDir: string = process.env.XDG_CONFIG_HOME
  ? path.join(process.env.XDG_CONFIG_HOME, _DEFINE_PROG)
  : path.join(os.homedir(), ".config", _DEFINE_PROG);

const cacheDir: string = process.env.XDG_CACHE_HOME
  ? path.join(process.env.XDG_CACHE_HOME, _DEFINE_PROG)
  : path.join(os.homedir(), ".cache", _DEFINE_PROG);

export const configFile: string = process.env.AKIBA_CONFIG
  ? process.env.AKIBA_CONFIG
  : path.join(configDir, "config.toml");

/*
 * Read configuration from file
 */
export function readConfig(): Config {
  let config: Config = {
    anidb: {
      url: "http://api.anidb.net:9001/httpapi",
      client: {},
      poster: false,
    },
    anilist: {},
    tmdb: {},
    cache: {
      path: cacheDir,
      anidb_age: 90,
      mapping_age: 7,
    },
    overwrite_nfo: false,
  };

  if (fs.existsSync(configFile) && fs.statSync(configFile).isFile()) {
    const configToml = toml.parse(
      fs.readFileSync(configFile, "utf8"),
    ) as Config;
    config = deepmerge(config, configToml);
  }

  return config;
}

/*
 * Write configuration to file
 */
export function writeConfig(config: Config): boolean {
  try {
    const configFilePath: string = path.dirname(configFile);
    if (!fs.existsSync(configFilePath)) {
      fs.mkdirSync(configFilePath, { recursive: true, mode: 0o750 });
    }
    fs.writeFileSync(configFile, toml.stringify(config), { encoding: "utf8" });
    fs.chmodSync(configFile, 0o600);
  } catch {
    return false;
  }

  return true;
}

/*
 * Ensure configuration is usable
 */
export function validateConfig(
  config: Config,
  verbose: boolean = false,
): boolean {
  let ret = true;
  if (config.anidb.url === undefined) {
    if (verbose) {
      banner();
      log("AniDB URL is missing from configration!", "error");
    }
    ret = false;
  }
  if (
    config.anidb.client.name === undefined ||
    config.anidb.client.version === undefined
  ) {
    if (verbose) {
      banner();
      log("AniDB Client Name/Version is not set!", "error");
      log(
        "Please register a HTTP client on https://anidb.net/software/add",
        "error",
      );
    }
    ret = false;
  }
  return ret;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
