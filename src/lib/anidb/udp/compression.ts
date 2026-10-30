/*
 * Decompression of datagrams sent when AUTH was called with comp=1
 */
import zlib from "node:zlib";

import { bytes } from "lib/anidb/udp/bytes";

/*
 * compressed datagrams start with two zero bytes, followed by a zlib stream
 */
export function isCompressed(buffer: Buffer): boolean {
  return buffer[0] === 0 && buffer[1] === 0;
}

/*
 * Inflates a compressed datagram. With `comp=1` in AUTH the server deflates
 * (DEFLATE, zlib) instead of truncating when a reply would not fit one datagram
 * (at most 1400 bytes).
 */
export function decompress(buffer: Buffer): Buffer {
  // Z_SYNC_FLUSH: return what was received instead of failing on a stream
  // that is cut off at the end of the datagram
  return zlib.inflateSync(bytes(buffer.subarray(2)), {
    finishFlush: zlib.constants.Z_SYNC_FLUSH,
  });
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
