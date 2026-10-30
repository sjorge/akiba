/*
 * Preload for bun test, points config and cache at a temp directory
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterAll } from "bun:test";

// NOTE: lib/config computes its paths at import time, so this must run first
const testRoot: string = fs.mkdtempSync(path.join(os.tmpdir(), "akiba-test-"));

process.env.XDG_CONFIG_HOME = path.join(testRoot, "config");
process.env.XDG_CACHE_HOME = path.join(testRoot, "cache");
process.env.AKIBA_CONFIG = path.join(testRoot, "config", "akiba.toml");

afterAll((): void => {
  fs.rmSync(testRoot, { recursive: true, force: true });
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
