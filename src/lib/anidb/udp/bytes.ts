/*
 * Typing helper for passing Buffers to node:crypto, node:zlib and node:dgram
 */

/*
 * The Buffer and Uint8Array typings in this project disagree, so a Buffer is
 * not accepted where a Uint8Array is expected. A Buffer is a Uint8Array at
 * runtime (and Bun's zlib only accepts real Buffers), so this only adjusts the
 * type.
 */
export function bytes(buffer: Buffer): Uint8Array {
  return buffer as unknown as Uint8Array;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
