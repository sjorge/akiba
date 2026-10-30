import fs from "node:fs";
import path from "node:path";
import { afterEach, expect, test } from "bun:test";

import {
  configFile,
  readConfig,
  validateConfig,
  writeConfig,
} from "lib/config";
import { makeConfig } from "test/helpers";

afterEach((): void => {
  fs.rmSync(path.dirname(configFile), { recursive: true, force: true });
});

test("readConfig returns defaults and merges a file", (): void => {
  expect(configFile).toBe(process.env.AKIBA_CONFIG as string);

  const defaults = readConfig();
  expect(defaults.anidb.http_client.url).toBe(
    "http://api.anidb.net:9001/httpapi",
  );
  expect(defaults.anidb.poster).toBe(false);
  expect(defaults.cache.path).toBe(
    path.join(process.env.XDG_CACHE_HOME as string, "akiba"),
  );
  expect(defaults.cache.metadata_age).toBe(90);
  expect(defaults.cache.mapping_age).toBe(7);
  expect(defaults.cache.hash_age).toBe(30);
  expect(defaults.renamer.format).toContain("{anime_name_romaji}");

  fs.mkdirSync(path.dirname(configFile), { recursive: true });
  fs.writeFileSync(
    configFile,
    '[anidb.udp_client]\nname = "client"\nversion = 2\n\n[cache]\nmapping_age = 1\n',
  );
  const merged = readConfig();
  expect(merged.anidb.udp_client).toEqual({ name: "client", version: 2 });
  expect(merged.cache.mapping_age).toBe(1);
  // untouched defaults survive the merge
  expect(merged.cache.hash_age).toBe(30);
});

test("writeConfig creates a private file", (): void => {
  expect(fs.existsSync(path.dirname(configFile))).toBe(false);
  expect(writeConfig(readConfig())).toBe(true);
  // NOTE: regression for creating the config file with mode 0o600
  expect(fs.statSync(configFile).mode & 0o777).toBe(0o600);

  // an existing file is tightened
  fs.chmodSync(configFile, 0o644);
  const config = readConfig();
  config.anilist.token = "secret";
  config.renamer.target_path = "/media/anime";
  writeConfig(config);
  expect(fs.statSync(configFile).mode & 0o777).toBe(0o600);

  // and it round trips
  const read = readConfig();
  expect(read.anilist.token).toBe("secret");
  expect(read.renamer.target_path).toBe("/media/anime");
});

test("validateConfig checks what each command needs", (): void => {
  const http = { anidb: { http_client: { name: "c", version: 1 } } };
  const udp = { name: "c", version: 1 };
  const creds = { ...udp, username: "u", password: "p" };

  // write-nfo needs an http client name and version
  expect(validateConfig(makeConfig(), "write-nfo")).toBe(false);
  expect(validateConfig(makeConfig(http), "write-nfo")).toBe(true);
  expect(
    validateConfig(
      makeConfig({ anidb: { http_client: { name: "c" } } }),
      "write-nfo",
    ),
  ).toBe(false);

  // renamer needs a udp client and credentials
  expect(validateConfig(makeConfig(), "renamer")).toBe(false);
  expect(
    validateConfig(makeConfig({ anidb: { udp_client: udp } }), "renamer"),
  ).toBe(false);
  expect(
    validateConfig(makeConfig({ anidb: { udp_client: creds } }), "renamer"),
  ).toBe(true);
  expect(
    validateConfig(
      makeConfig({
        anidb: { udp_client: { name: "c", username: "u", password: "p" } },
      }),
      "renamer",
    ),
  ).toBe(false);

  // the commands do not depend on each other
  expect(validateConfig(makeConfig(http), "renamer")).toBe(false);
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
