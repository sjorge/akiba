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
    http_client: {
      url: string;
      name?: string;
      version?: number;
    };
    udp_client: {
      host: string;
      name?: string;
      version?: number;
      username?: string;
      password?: string;
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
    metadata_age: number;
    mapping_age: number;
    hash_age: number;
  };
  renamer: {
    format: string;
    targetPath?: string;
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
      http_client: { url: "http://api.anidb.net:9001/httpapi" },
      udp_client: { host: "api.anidb.net:9000" },
      poster: false,
    },
    anilist: {},
    tmdb: {},
    cache: {
      path: cacheDir,
      metadata_age: 90,
      mapping_age: 7,
      hash_age: 30,
    },
    renamer: {
      format:
        "{anime_name_romaji}/{anime_name_romaji} - {episode} - {episode_name} ({crc32}).{filetype}",
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
  command: "write-nfo" | "renamer",
  verbose: boolean = false,
): boolean {
  let ret = true;

  if (command == "write-nfo") {
    if (config.anidb.http_client.url === undefined) {
      if (verbose) {
        banner();
        log("Please configure the AniDB URL!", "error");
      }
      ret = false;
    }

    if (
      config.anidb.http_client.name === undefined ||
      config.anidb.http_client.version === undefined
    ) {
      if (verbose) {
        banner();
        log(
          "AniDB HTTP Client not configured! Please set with --anidb-http-client",
          "error",
        );
        log(
          "Please register a project on https://anidb.net/software/add, and add a HTTP client",
          "error",
        );
      }
      ret = false;
    }
  }
  if (command == "renamer") {
    if (config.anidb.udp_client.host === undefined) {
      if (verbose) {
        banner();
        log("Please configure the AniDB Host!", "error");
      }
      ret = false;
    }
    if (
      config.anidb.udp_client.name === undefined ||
      config.anidb.udp_client.version === undefined
    ) {
      if (verbose) {
        banner();
        log(
          "AniDB UDP Client not configured! Please set with --anidb-udp-client.",
          "error",
        );
        log(
          "Please register a project on https://anidb.net/software/add, and add a UDP client",
          "error",
        );
      }
      ret = false;
    }
    if (
      config.anidb.udp_client.username === undefined ||
      config.anidb.udp_client.password === undefined
    ) {
      if (verbose) {
        banner();
        log(
          "AniDB UDP Client missing username and password configuration! Please set with --anidb-auth-username and --anidb-auth-password.",
          "error",
        );
      }
      ret = false;
    }
  }
  return ret;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
