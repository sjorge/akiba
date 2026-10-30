import fs from "node:fs";
import path from "node:path";
import { expect, test } from "bun:test";

import { makeTempDir } from "test/helpers";

type CliResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

async function runCli(args: string[], root: string): Promise<CliResult> {
  const proc = Bun.spawn(["bun", "run", "src/cli.ts", ...args], {
    env: {
      ...process.env,
      XDG_CONFIG_HOME: path.join(root, "config"),
      XDG_CACHE_HOME: path.join(root, "cache"),
      AKIBA_CONFIG: path.join(root, "config", "akiba.toml"),
    },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  return { exitCode, stdout, stderr };
}

test("cli --help, --version and argument validation", async (): Promise<void> => {
  const root = makeTempDir();

  const help = await runCli(["--help"], root);
  expect(help.exitCode).toBe(0);
  for (const command of ["configure", "write-nfo", "rename", "idmap"]) {
    expect(help.stdout).toContain(command);
  }

  const version = await runCli(["--version"], root);
  expect(version.exitCode).toBe(0);
  expect(version.stdout.trim()).toMatch(/^\d+\.\d+\.\d+$/);

  const invalid = await runCli(
    ["configure", "--rename-format", "{nope}"],
    root,
  );
  expect(invalid.exitCode).not.toBe(0);
  expect(invalid.stderr).toContain("The tag {nope} is not known");
});

test("cli configure writes the config and dumps it", async (): Promise<void> => {
  const root = makeTempDir();
  const result = await runCli(
    ["configure", "--anidb-udp-client", "test/1", "--dump"],
    root,
  );

  const configFile = path.join(root, "config", "akiba.toml");
  expect(fs.readFileSync(configFile, "utf8")).toContain('name = "test"');
  expect(fs.statSync(configFile).mode & 0o777).toBe(0o600);

  // NOTE: the dump is intentionally the full config, secrets included
  const dump = JSON.parse(result.stdout.slice(result.stdout.indexOf("{")));
  expect(dump.anidb.udp_client).toEqual({ name: "test", version: 1 });

  // the rest of the configuration is still missing
  expect(result.exitCode).toBe(1);
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
