import { expect, spyOn, test } from "bun:test";
import { Command } from "@commander-js/extra-typings";

import * as logger from "lib/logger";
import { addConfigureCommand } from "cmd/configure";
import { readConfig, writeConfig } from "lib/config";
import { makeConfig, rejectionMessage } from "test/helpers";

/*
 * Run `configure` through commander, the messages it logged are discarded
 */
async function runConfigure(args: string[]): Promise<void> {
  const program = new Command()
    .exitOverride()
    .configureOutput({ writeOut: () => {}, writeErr: () => {} });
  addConfigureCommand(program);

  const log = spyOn(logger, "log").mockImplementation((): void => {});
  const banner = spyOn(logger, "banner").mockImplementation((): void => {});
  const previousExitCode = process.exitCode;
  try {
    await program.parseAsync(["configure", ...args], { from: "user" });
  } finally {
    // NOTE: Bun ignores an undefined exit code, so 0 is the reset value
    process.exitCode = previousExitCode ?? 0;
    log.mockRestore();
    banner.mockRestore();
  }
}

test("configure rejects client versions that are not whole integers >= 0", async (): Promise<void> => {
  for (const flag of ["--anidb-udp-client", "--anidb-http-client"]) {
    for (const bad of [
      "test/-1",
      "test/1.5",
      "test/12x",
      "test/1e3",
      "test/",
      "test/4294967296",
    ]) {
      expect(await rejectionMessage(runConfigure([flag, bad]))).toContain(
        "Expecting a number for version.",
      );
    }
  }
});

test("configure stores a whole integer client version", async (): Promise<void> => {
  expect(writeConfig(makeConfig())).toBe(true);

  await runConfigure([
    "--anidb-udp-client",
    "test/0",
    "--anidb-http-client",
    "test/12",
  ]);

  const stored = readConfig();
  expect(stored.anidb.udp_client).toMatchObject({ name: "test", version: 0 });
  expect(stored.anidb.http_client).toMatchObject({ name: "test", version: 12 });
});

test("configure rejects udp client names that are not 4 to 16 lower case letters", async (): Promise<void> => {
  for (const bad of ["abc", "Akiba", "my_client", "client1", "a".repeat(17)]) {
    for (const suffix of ["", "/3"]) {
      expect(
        await rejectionMessage(
          runConfigure(["--anidb-udp-client", `${bad}${suffix}`]),
        ),
      ).toContain("lower case letters");
    }
  }
});

test("configure leaves the http client name unchecked", async (): Promise<void> => {
  expect(writeConfig(makeConfig())).toBe(true);
  await runConfigure(["--anidb-http-client", "my-client2/1"]);
  expect(readConfig().anidb.http_client.name).toBe("my-client2");
});

test("configure accepts the shortest and longest client names", async (): Promise<void> => {
  expect(writeConfig(makeConfig())).toBe(true);
  await runConfigure([
    "--anidb-udp-client",
    "abcd/2",
    "--anidb-http-client",
    `${"z".repeat(16)}/3`,
  ]);
  const stored = readConfig();
  expect(stored.anidb.udp_client).toMatchObject({ name: "abcd", version: 2 });
  expect(stored.anidb.http_client.name).toBe("z".repeat(16));
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
