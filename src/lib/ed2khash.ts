/*
 * Internal helper types and function for ed2k hashing
 */
import fs from "node:fs";
import path from "node:path";
import { createMD4 } from "hash-wasm";

export type Ed2kHash = {
  hash: string;
  size: number;
  link: string;
  createdAt: number;
};

export async function generateEd2kHash(file: string): Promise<Ed2kHash> {
  // sanity check parameters
  if (!fs.existsSync(file)) throw new Error(`${file} does not exist!`);
  if (!fs.statSync(file).isFile()) throw new Error(`${file} must be a file!`);

  async function hash(file: string): Promise<string> {
    // https://wiki.anidb.net/Ed2k-hash
    const md4 = await createMD4();
    const blockSize = 9728000;
    const blockHashes: string[] = [];
    let buffer = Buffer.alloc(0);

    const fileStream = fs.createReadStream(file);

    for await (const chunk of fileStream) {
      buffer = Buffer.concat([buffer, chunk]);

      // Process full blocks
      while (buffer.length >= blockSize) {
        const block = buffer.subarray(0, blockSize);
        buffer = buffer.subarray(blockSize);

        md4.init();
        md4.update(block);

        blockHashes.push(md4.digest());
      }
    }

    // Process remaining data (last chunk)
    if (buffer.length > 0) {
      md4.init();
      md4.update(buffer);
      blockHashes.push(md4.digest());
    }

    // return fast as we have a single block
    if (blockHashes.length === 1) return blockHashes[0];

    // compute hash of all block hashes if we have multiple
    md4.init();
    for (const hash of blockHashes) {
      md4.update(Buffer.from(hash, "hex"));
    }
    return md4.digest();
  }

  // generate hash
  const fileHash: string = await hash(file);
  const fileSize: number = fs.statSync(file).size;

  return {
    hash: fileHash,
    size: fileSize,
    link: `ed2k://|file|${path.basename(file)}|${fileSize}|${fileHash}|/`,
    createdAt: Date.now(),
  } as Ed2kHash;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
