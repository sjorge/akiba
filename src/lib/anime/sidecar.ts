/*
 * Sidecar files (subtitles and the like) that follow their episode
 */
import fs from "node:fs";
import path from "node:path";
import { ANIME_EPISODE_EXTENSIONS } from "lib/anime";

// NOTE: the file name limit of ext4, APFS, ZFS and SMB
export const NAME_MAX = 255;

// extensions of the sidecar files that follow their episode, matched
// case-insensitively. Add a format by adding its extension here.
const SIDECAR_EXTENSIONS: string[] = [
  "nfo",
  "srt",
  "ass",
  "ssa",
  "vtt",
  "sub",
  "idx",
  "sup",
  "mks",
];

/*
 * The file name of a sidecar of the episode `stem`: the stem and the `tail`
 * (`.nfo`, `.en.srt`, ...) that followed the stem on disk
 */
export function sidecarName(stem: string, tail: string): string {
  return `${stem}${tail}`;
}

/*
 * `sidecarName` for the destination. A name the file system refuses as too
 * long gets its stem cut and marked with `~`, the way episode names are cut.
 * The tail stays as it is.
 */
export function fitSidecarName(stem: string, tail: string): string {
  const name = sidecarName(stem, tail);
  if (Buffer.byteLength(name) <= NAME_MAX) return name;

  const budget = Math.max(0, NAME_MAX - Buffer.byteLength(tail) - 1);
  let cut = "";
  let bytes = 0;
  for (const char of stem) {
    bytes += Buffer.byteLength(char);
    if (bytes > budget) break;
    cut += char;
  }
  return `${cut.trimEnd()}~${tail}`;
}

/*
 * Whether `name` is `stem` followed by a `.`: `Show - 1.5` continues
 * `Show - 1`, `Show - 10` does not
 */
function continues(name: string, stem: string): boolean {
  return name.length > stem.length && name.startsWith(`${stem}.`);
}

/*
 * The tail of a sidecar named `name` for the episode `stem`: the part after
 * the stem, starting with its `.`, with the extension last (`.srt`,
 * `.en.srt`, `.eng.forced.ass`)
 */
function sidecarTail(name: string, stem: string): string | undefined {
  if (!continues(name, stem)) return undefined;
  const tail = name.substring(stem.length);
  const extension = tail.substring(tail.lastIndexOf(".") + 1).toLowerCase();
  return SIDECAR_EXTENSIONS.includes(extension) ? tail : undefined;
}

/*
 * Whether `name` has an episode extension. Unlike the directory walk this
 * ignores case: a longer episode such as `Show - 1.5.MKV` must still keep its
 * subtitles away from `Show - 1`.
 */
function isEpisodeName(name: string): boolean {
  return ANIME_EPISODE_EXTENSIONS.includes(path.extname(name).toLowerCase());
}

/*
 * The tails of the sidecar files of the episode `source`, as spelled on disk,
 * sorted. The episode itself and directories are skipped. A file that
 * continues the stem of a longer episode next to it is left to that episode:
 * `Show - 1.5.srt` belongs to `Show - 1.5.mkv`, not to `Show - 1.mkv`.
 */
export function findSidecars(source: string, stem: string): string[] {
  const dir = path.dirname(source);
  let entries: fs.Dirent[];
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return [];
  }
  const files = entries
    .filter((entry: fs.Dirent) => !entry.isDirectory())
    .map((entry: fs.Dirent) => entry.name);

  const longer = files
    .filter(isEpisodeName)
    .map((name: string) => path.parse(name).name)
    .filter((episode: string) => continues(episode, stem));

  return files
    .filter((name: string) => name !== path.basename(source))
    .filter(
      (name: string) =>
        !longer.some((episode: string) => continues(name, episode)),
    )
    .map((name: string) => sidecarTail(name, stem))
    .filter((tail: string | undefined): tail is string => tail !== undefined)
    .sort();
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
