import fs from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";
import { expect, spyOn, test } from "bun:test";

import { writeFileAtomic, writeStreamAtomic } from "lib/fs";
import { makeTempDir } from "test/helpers";

function mode(file: string): number {
  return fs.statSync(file).mode & 0o777;
}

// a stream that sends some bytes and then fails, like a dropped connection
function brokenStream(): Readable {
  return new Readable({
    read(): void {
      this.push("partial");
      this.destroy(new Error("connection reset"));
    },
  });
}

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

test("writeFileAtomic uses the requested mode whatever the umask", (): void => {
  const dir = makeTempDir();
  const file = path.join(dir, "tvshow.nfo");

  const previous = process.umask(0o077);
  try {
    writeFileAtomic(file, "<tvshow/>", 0o644);
  } finally {
    process.umask(previous);
  }

  expect(mode(file)).toBe(0o644);
});

test("writeStreamAtomic writes the stream and leaves no tmp file", async (): Promise<void> => {
  const dir = makeTempDir();
  const file = path.join(dir, "poster.jpg");

  const previous = process.umask(0o077);
  try {
    await writeStreamAtomic(file, Readable.from([Buffer.from("JPEG")]), 0o644);
  } finally {
    process.umask(previous);
  }

  expect(fs.readFileSync(file, "utf8")).toBe("JPEG");
  expect(mode(file)).toBe(0o644);
  expect(fs.readdirSync(dir)).toEqual(["poster.jpg"]);
});

test("writeStreamAtomic keeps the old file when the stream fails", async (): Promise<void> => {
  const dir = makeTempDir();
  const file = path.join(dir, "poster.jpg");
  fs.writeFileSync(file, "OLD");

  let failure: unknown;
  try {
    await writeStreamAtomic(file, brokenStream(), 0o644);
  } catch (e: unknown) {
    failure = e;
  }

  expect(failure).toBeInstanceOf(Error);
  expect(fs.readFileSync(file, "utf8")).toBe("OLD");
  expect(fs.readdirSync(dir)).toEqual(["poster.jpg"]);
});

test("writeStreamAtomic keeps the old file when the rename fails", async (): Promise<void> => {
  const dir = makeTempDir();
  const file = path.join(dir, "poster.jpg");
  fs.writeFileSync(file, "OLD");

  const failing = spyOn(fs, "renameSync").mockImplementation((): void => {
    throw new Error("EIO: input/output error");
  });
  let failure: unknown;
  try {
    await writeStreamAtomic(file, Readable.from([Buffer.from("NEW")]), 0o644);
  } catch (e: unknown) {
    failure = e;
  } finally {
    failing.mockRestore();
  }

  expect(failure).toBeInstanceOf(Error);
  expect((failure as Error).message).toContain("EIO");

  expect(fs.readFileSync(file, "utf8")).toBe("OLD");
  expect(fs.readdirSync(dir)).toEqual(["poster.jpg"]);
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
