/*
 * Internal helpers for file system writes
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

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
  const base = `${path.basename(file)}.${process.pid}.tmp`;
  const staged = path.join(os.tmpdir(), base);
  const tmp = path.join(path.dirname(file), base);

  try {
    fs.writeFileSync(staged, data, { encoding: "utf8", mode });
    fs.copyFileSync(staged, tmp);
    fs.chmodSync(tmp, mode);
    // NOTE: rename within one directory is atomic on local filesystems and on
    // SMB/NFS shares, which is why the copy lives next to the target
    fs.renameSync(tmp, file);
  } catch (_e: unknown) {
    try {
      fs.rmSync(tmp, { force: true });
    } catch {
      // best effort, the original error is more useful
    }
    throw _e;
  } finally {
    try {
      fs.rmSync(staged, { force: true });
    } catch {
      // best effort
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
