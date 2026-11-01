import fs from "node:fs";
import path from "node:path";
import { expect, spyOn, test } from "bun:test";

import * as logger from "lib/logger";
import { renameAction } from "cmd/rename";
import { AnidbError, AnidbUDPClient, RESPONSE_CODE } from "lib/anidb";
import type { MylistAddByFileResult } from "lib/anidb";
import type { Config } from "lib/config";
import { configFile, writeConfig } from "lib/config";
import { generateEd2kHash } from "lib/ed2khash";
import {
  makeConfig,
  makeTempDir,
  sampleFormatData,
  seedCacheFile,
} from "test/helpers";

// NOTE: writes the config for a run that renames into target
function writeRenameConfig(target: string, format: string): Config {
  const config = makeConfig({
    anidb: {
      udp_client: { name: "client", version: 1, username: "u", password: "p" },
    },
    renamer: { target_path: target, format },
  });
  expect(writeConfig(config)).toBe(true);
  return config;
}

// NOTE: the file is served from the episode cache, so AniDB is not asked for it
async function seedKnownFile(
  config: Config,
  file: string,
  romaji: string,
): Promise<void> {
  const hash = (await generateEd2kHash(file)).hash;
  seedCacheFile(
    config.cache.path,
    path.join("anidb", `${hash}.fid.toml`),
    sampleFormatData({ anime_name_romaji: romaji }),
  );
}

// NOTE: captures what a run writes to stdout and stderr
function captureOutput(): { text: () => string; restore: () => void } {
  const stdout = spyOn(process.stdout, "write").mockReturnValue(true);
  const stderr = spyOn(process.stderr, "write").mockReturnValue(true);
  return {
    text: () =>
      [...stdout.mock.calls, ...stderr.mock.calls]
        .map((call) => String(call[0]))
        .join(""),
    restore: (): void => {
      stderr.mockRestore();
      stdout.mockRestore();
    },
  };
}

// NOTE: only the AniDB network calls are faked, hashing, the unknown hash
//       handling, formatting and renaming are the real thing. The known file is
//       served from the episode cache, using a real record from test/fixtures
test("rename skips a file unknown to AniDB and still renames the next", async (): Promise<void> => {
  const dir = makeTempDir();
  const target = path.join(dir, "target");
  fs.mkdirSync(target);
  // sorts first, so it is the one AniDB does not know
  const unknown = path.join(dir, "a.mkv");
  const known = path.join(dir, "b.mkv");
  fs.writeFileSync(unknown, "unknown-content");
  fs.writeFileSync(known, "known");

  const config = makeConfig({
    anidb: {
      udp_client: { name: "client", version: 1, username: "u", password: "p" },
    },
    renamer: { target_path: target, format: "{anime_name_romaji}.{filetype}" },
  });
  expect(writeConfig(config)).toBe(true);
  const knownHash = (await generateEd2kHash(known)).hash;
  seedCacheFile(
    config.cache.path,
    path.join("anidb", `${knownHash}.fid.toml`),
    sampleFormatData({ anime_name_romaji: "Test Anime" }),
  );

  const connect = spyOn(AnidbUDPClient.prototype, "connect").mockResolvedValue(
    undefined,
  );
  const disconnect = spyOn(
    AnidbUDPClient.prototype,
    "disconnect",
  ).mockResolvedValue(undefined);
  const lookup = spyOn(
    AnidbUDPClient.prototype,
    "file_by_hash",
  ).mockRejectedValue(new AnidbError(RESPONSE_CODE.NO_SUCH_FILE));
  const banner = spyOn(logger, "banner").mockImplementation(() => {});
  const stdout = spyOn(process.stdout, "write").mockReturnValue(true);
  const stderr = spyOn(process.stderr, "write").mockReturnValue(true);
  const previousExitCode = process.exitCode;

  try {
    await renameAction(dir, { dryRun: true });
    const output = [...stdout.mock.calls, ...stderr.mock.calls]
      .map((call) => String(call[0]))
      .join("");

    // AniDB was only asked about the unknown file, which sorts first
    expect(lookup.mock.calls.map((call) => call[1])).toEqual([
      fs.statSync(unknown).size,
    ]);
    // the unknown file is reported and flagged, but does not stop the run
    expect(output).toContain("a.mkv: Identifying ...");
    expect(output).not.toContain("not processed");
    expect(process.exitCode).toBe(1);
    // the known file got its new name
    expect(output).toContain(
      `b.mkv: Moved to ${path.join(target, "Test Anime.mkv")}`,
    );
    // the unknown file was left alone, and no lookup was treated as a failure
    expect(fs.existsSync(unknown)).toBe(true);
    expect(output).not.toContain("AniDB UDP Client:");
  } finally {
    // NOTE: Bun ignores an undefined exit code, so 0 is the reset value
    process.exitCode = previousExitCode ?? 0;
    banner.mockRestore();
    stderr.mockRestore();
    stdout.mockRestore();
    lookup.mockRestore();
    disconnect.mockRestore();
    connect.mockRestore();
    fs.rmSync(configFile, { force: true });
  }
});

test("rename continues after a file fails to rename, and exits 1", async (): Promise<void> => {
  const dir = makeTempDir();
  const target = path.join(dir, "target");
  fs.mkdirSync(target);
  // a regular file where the directory of the first destination belongs, so
  // only the first episode cannot be renamed
  fs.writeFileSync(path.join(target, "Show A"), "in the way");
  const first = path.join(dir, "a.mkv");
  const second = path.join(dir, "b.mkv");
  fs.writeFileSync(first, "first");
  fs.writeFileSync(second, "second");

  const config = writeRenameConfig(
    target,
    "{anime_name_romaji}/{anime_name_romaji}.{filetype}",
  );
  await seedKnownFile(config, first, "Show A");
  await seedKnownFile(config, second, "Show B");

  const connect = spyOn(AnidbUDPClient.prototype, "connect").mockResolvedValue(
    undefined,
  );
  const disconnect = spyOn(
    AnidbUDPClient.prototype,
    "disconnect",
  ).mockResolvedValue(undefined);
  // only a safety net, every file is served from the cache
  const lookup = spyOn(
    AnidbUDPClient.prototype,
    "file_by_hash",
  ).mockRejectedValue(new AnidbError(RESPONSE_CODE.NO_SUCH_FILE));
  const banner = spyOn(logger, "banner").mockImplementation(() => {});
  const output = captureOutput();
  const previousExitCode = process.exitCode;

  try {
    await renameAction(dir, {});

    expect(lookup).not.toHaveBeenCalled();
    // the failure is reported, but does not stop the run
    expect(output.text()).toContain("a.mkv: Renaming ...");
    expect(output.text()).not.toContain("not processed");
    expect(process.exitCode).toBe(1);
    // the file that failed stays in place, the next one was renamed
    expect(fs.existsSync(first)).toBe(true);
    expect(fs.existsSync(second)).toBe(false);
    expect(output.text()).toContain(
      `b.mkv: Moved to ${path.join(target, "Show B", "Show B.mkv")}`,
    );
    expect(fs.existsSync(path.join(target, "Show B", "Show B.mkv"))).toBe(true);
  } finally {
    // NOTE: Bun ignores an undefined exit code, so 0 is the reset value
    process.exitCode = previousExitCode ?? 0;
    banner.mockRestore();
    output.restore();
    lookup.mockRestore();
    disconnect.mockRestore();
    connect.mockRestore();
    fs.rmSync(configFile, { force: true });
  }
});

test("rename exits 1 when AniDB does not confirm the mylist state, and still renames", async (): Promise<void> => {
  const dir = makeTempDir();
  const target = path.join(dir, "target");
  fs.mkdirSync(target);
  const file = path.join(dir, "a.mkv");
  fs.writeFileSync(file, "known");

  const config = writeRenameConfig(target, "{anime_name_romaji}.{filetype}");
  await seedKnownFile(config, file, "Test Anime");

  const connect = spyOn(AnidbUDPClient.prototype, "connect").mockResolvedValue(
    undefined,
  );
  const disconnect = spyOn(
    AnidbUDPClient.prototype,
    "disconnect",
  ).mockResolvedValue(undefined);
  const lookup = spyOn(
    AnidbUDPClient.prototype,
    "file_by_hash",
  ).mockRejectedValue(new AnidbError(RESPONSE_CODE.NO_SUCH_FILE));
  // the file is not in the mylist yet, and the update is not confirmed
  const mylistLookup = spyOn(
    AnidbUDPClient.prototype,
    "mylist_by_file_hash",
  ).mockRejectedValue(new AnidbError(RESPONSE_CODE.NO_SUCH_ENTRY));
  const mylistAdd = spyOn(
    AnidbUDPClient.prototype,
    "mylist_add_by_file",
  ).mockResolvedValue(undefined as unknown as MylistAddByFileResult);
  const banner = spyOn(logger, "banner").mockImplementation(() => {});
  const output = captureOutput();
  const previousExitCode = process.exitCode;

  try {
    await renameAction(dir, { mylistState: "internal_storage" });

    expect(lookup).not.toHaveBeenCalled();
    expect(mylistLookup).toHaveBeenCalled();
    expect(mylistAdd).toHaveBeenCalled();
    expect(output.text()).toContain(
      "a.mkv: Updating mylist state to internal_storage ...",
    );
    expect(process.exitCode).toBe(1);
    // the file is still renamed
    expect(fs.existsSync(file)).toBe(false);
    expect(output.text()).toContain(
      `a.mkv: Moved to ${path.join(target, "Test Anime.mkv")}`,
    );
  } finally {
    // NOTE: Bun ignores an undefined exit code, so 0 is the reset value
    process.exitCode = previousExitCode ?? 0;
    banner.mockRestore();
    output.restore();
    mylistAdd.mockRestore();
    mylistLookup.mockRestore();
    lookup.mockRestore();
    disconnect.mockRestore();
    connect.mockRestore();
    fs.rmSync(configFile, { force: true });
  }
});

test("rename stops after an AniDB ban and reports the files not processed", async (): Promise<void> => {
  const dir = makeTempDir();
  const target = path.join(dir, "target");
  fs.mkdirSync(target);
  const first = path.join(dir, "a.mkv");
  const second = path.join(dir, "b.mkv");
  fs.writeFileSync(first, "first");
  fs.writeFileSync(second, "second");

  writeRenameConfig(target, "{anime_name_romaji}.{filetype}");

  const connect = spyOn(AnidbUDPClient.prototype, "connect").mockResolvedValue(
    undefined,
  );
  const disconnect = spyOn(
    AnidbUDPClient.prototype,
    "disconnect",
  ).mockResolvedValue(undefined);
  const lookup = spyOn(
    AnidbUDPClient.prototype,
    "file_by_hash",
  ).mockRejectedValue(new AnidbError(RESPONSE_CODE.BANNED));
  const banner = spyOn(logger, "banner").mockImplementation(() => {});
  const output = captureOutput();
  const previousExitCode = process.exitCode;

  try {
    await renameAction(dir, {});

    // a ban makes every later lookup fail too, so only the first file is tried
    expect(lookup).toHaveBeenCalledTimes(1);
    expect(output.text()).toContain("Stopped, 1 file(s) not processed.");
    expect(process.exitCode).toBe(1);
    expect(fs.existsSync(first)).toBe(true);
    expect(fs.existsSync(second)).toBe(true);
  } finally {
    // NOTE: Bun ignores an undefined exit code, so 0 is the reset value
    process.exitCode = previousExitCode ?? 0;
    banner.mockRestore();
    output.restore();
    lookup.mockRestore();
    disconnect.mockRestore();
    connect.mockRestore();
    fs.rmSync(configFile, { force: true });
  }
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
