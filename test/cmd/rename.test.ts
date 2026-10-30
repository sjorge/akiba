import fs from "node:fs";
import path from "node:path";
import { expect, spyOn, test } from "bun:test";

import * as logger from "lib/logger";
import { renameAction } from "cmd/rename";
import { AnidbError, AnidbUDPClient, RESPONSE_CODE } from "lib/anidb";
import { configFile, writeConfig } from "lib/config";
import { generateEd2kHash } from "lib/ed2khash";
import {
  makeConfig,
  makeTempDir,
  sampleFormatData,
  seedCacheFile,
} from "test/helpers";

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
    process.exitCode = previousExitCode;
    banner.mockRestore();
    stderr.mockRestore();
    stdout.mockRestore();
    lookup.mockRestore();
    disconnect.mockRestore();
    connect.mockRestore();
    fs.rmSync(configFile, { force: true });
  }
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
