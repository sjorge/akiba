/*
 * Internal helpers for file system writes
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";

/*
 * Name of the temporary file that is written next to `file` before it is
 * renamed over it.
 */
function tempName(file: string): string {
  return `${path.basename(file)}.${process.pid}.tmp`;
}

function removeQuietly(file: string): void {
  try {
    fs.rmSync(file, { force: true });
  } catch {
    // best effort, the original error is more useful
  }
}

/*
 * Write `data` to `file` so that readers see either the old or the new
 * content, never a truncated file. The data is first staged in the local
 * temporary directory, so a failure there never touches the (possibly remote)
 * target directory. It is then copied to a temporary file next to `file`,
 * which is renamed over it. Both temporary files are removed when a step
 * fails, and the error is rethrown.
 */
export function writeFileAtomic(
  file: string,
  data: string,
  mode: number,
): void {
  const staged = path.join(os.tmpdir(), tempName(file));
  const tmp = path.join(path.dirname(file), tempName(file));

  try {
    fs.writeFileSync(staged, data, { encoding: "utf8", mode });
    fs.copyFileSync(staged, tmp);
    fs.chmodSync(tmp, mode);
    // NOTE: rename within one directory is atomic on local filesystems and on
    // SMB/NFS shares, which is why the copy lives next to the target
    fs.renameSync(tmp, file);
  } catch (_e: unknown) {
    removeQuietly(tmp);
    throw _e;
  } finally {
    removeQuietly(staged);
  }
}

/*
 * Write the bytes of `source` to `file` so that readers see either the old or
 * the new content. The stream is written to a temporary file next to `file`,
 * which is renamed over it once the whole stream has arrived. A stream that
 * fails part way, or a failed rename, removes the temporary file and rethrows,
 * so the old file is kept.
 */
export async function writeStreamAtomic(
  file: string,
  source: Readable,
  mode: number,
): Promise<void> {
  const tmp = path.join(path.dirname(file), tempName(file));

  try {
    await pipeline(source, fs.createWriteStream(tmp, { mode }));
    fs.chmodSync(tmp, mode);
    fs.renameSync(tmp, file);
  } catch (_e: unknown) {
    removeQuietly(tmp);
    throw _e;
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
