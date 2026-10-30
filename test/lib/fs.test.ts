import fs from "node:fs";
import path from "node:path";
import { expect, spyOn, test } from "bun:test";

import { writeFileAtomic } from "lib/fs";
import { makeTempDir } from "test/helpers";

test("writeFileAtomic writes the content and leaves no tmp file", (): void => {
  const dir = makeTempDir();
  const file = path.join(dir, "cache.toml");

  writeFileAtomic(file, "a = 1\n", 0o660);

  expect(fs.readFileSync(file, "utf8")).toBe("a = 1\n");
  expect(fs.readdirSync(dir)).toEqual(["cache.toml"]);
});

test("writeFileAtomic keeps the old file when the rename fails", (): void => {
  const dir = makeTempDir();
  const file = path.join(dir, "cache.toml");
  fs.writeFileSync(file, "old");

  // the rename is the step that replaces the target, make it fail
  const failing = spyOn(fs, "renameSync").mockImplementation((): void => {
    throw new Error("EIO: input/output error");
  });
  try {
    expect(() => writeFileAtomic(file, "new", 0o660)).toThrow("EIO");
  } finally {
    failing.mockRestore();
  }

  expect(fs.readFileSync(file, "utf8")).toBe("old");
  expect(fs.readdirSync(dir)).toEqual(["cache.toml"]);
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
