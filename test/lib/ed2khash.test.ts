import fs from "node:fs";
import path from "node:path";
import { expect, test } from "bun:test";
import { md4 } from "hash-wasm";

import { generateEd2kHash } from "lib/ed2khash";
import { makeTempDir } from "test/helpers";

const BLOCK_SIZE = 9728000;

test("generateEd2kHash hashes small and multi block files", async (): Promise<void> => {
  const dir = makeTempDir();

  // a single block is the md4 of the content, including the link and size
  const small = path.join(dir, "episode.mkv");
  fs.writeFileSync(small, "abc");
  const result = await generateEd2kHash(small);
  expect(result.hash).toBe("a448017aaf21d8525fc10ae87aa6729d");
  expect(result.size).toBe(3);
  expect(result.link).toBe(
    "ed2k://|file|episode.mkv|3|a448017aaf21d8525fc10ae87aa6729d|/",
  );

  // multiple blocks hash the concatenated block digests
  // NOTE: a file of exactly one block is not covered, the AniDB wiki has two
  //       variants for exact multiples of the block size ("red" and "blue")
  const content = new Uint8Array(BLOCK_SIZE + 1).fill(0x61);
  const large = path.join(dir, "large.mkv");
  fs.writeFileSync(large, content);
  const first = await md4(content.subarray(0, BLOCK_SIZE));
  const second = await md4(content.subarray(BLOCK_SIZE));
  const expected = await md4(
    Uint8Array.from([
      ...Buffer.from(first, "hex"),
      ...Buffer.from(second, "hex"),
    ]),
  );
  expect((await generateEd2kHash(large)).hash).toBe(expected);
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
