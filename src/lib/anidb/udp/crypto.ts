/*
 * AES-128 encryption of the AniDB UDP session (ENCRYPT command)
 */
import crypto from "node:crypto";
import { bytes } from "lib/anidb/udp/bytes";

const ALGORITHM: string = "aes-128-ecb";

function concat(a: Buffer, b: Buffer): Buffer {
  return Buffer.concat([bytes(a), bytes(b)]);
}

export function createKey(udpApiKey: string, salt: string): Buffer {
  return crypto
    .createHash("md5")
    .update(udpApiKey + salt)
    .digest();
}

export function encrypt(input: string | Buffer, key: Buffer): Buffer {
  const cipher = crypto.createCipheriv(ALGORITHM, bytes(key), null);
  cipher.setAutoPadding(true);
  return concat(
    cipher.update(typeof input === "string" ? input : bytes(input)),
    cipher.final(),
  );
}

export function decrypt(input: Buffer, key: Buffer): Buffer {
  const decipher = crypto.createDecipheriv(
    ALGORITHM,
    bytes(key),
    null,
  );
  decipher.setAutoPadding(true);
  return concat(decipher.update(bytes(input)), decipher.final());
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
