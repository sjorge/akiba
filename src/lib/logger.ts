import { _DEFINE_PROG, _DEFINE_VER } from "vars";
import tty from "node:tty";
import process from "node:process";
import type { AnimeId } from "lib/anime";

let printBanner = true;

function idSection(id?: AnimeId): string {
  const useColor: boolean = tty.isatty(process.stdout.fd);
  const anidb = id?.anidb ? (useColor ? "\x1b[31m#\x1b[0m" : "#") : " ";
  const anilist = id?.anilist ? (useColor ? "\x1b[34m#\x1b[0m" : "#") : " ";
  const tmdb = id?.tmdb ? (useColor ? "\x1b[36m#\x1b[0m" : "#") : " ";
  return id === undefined
    ? "{   |      } "
    : `{${anidb}${anilist}${tmdb}|${id.anidb.toString().padStart(6)}} `;
}

export function log(
  msg: string,
  type: "error" | "warn" | "step" | "done" | "info" = "info",
  showAnimeIds: boolean = false,
  anime?: AnimeId,
): void {
  const useColor: boolean = tty.isatty(process.stdout.fd);
  const ids = showAnimeIds ? idSection(anime) : "";
  switch (type) {
    case "error":
      if (useColor) {
        process.stderr.write(`\x1b[2K\r[\x1b[31m!!\x1b[0m] ${ids}${msg}\n`);
      } else {
        process.stdout.write(`[!!] ${ids}${msg}\n`);
      }
      break;
    case "warn":
      if (useColor) {
        process.stdout.write(`\x1b[2K\r[\x1b[33mWW\x1b[0m] ${ids}${msg}\n`);
      } else {
        process.stdout.write(`[WW] ${ids}${msg}\n`);
      }
      break;
    case "info":
      if (useColor) {
        process.stdout.write(`\x1b[2K\r[\x1b[34mII\x1b[0m] ${ids}${msg}\n`);
      } else {
        process.stdout.write(`[II] ${ids}${msg}\n`);
      }
      break;
    case "done":
      if (useColor) {
        process.stdout.write(`\x1b[2K\r[\x1b[32mOK\x1b[0m] ${ids}${msg}\n`);
      } else {
        process.stdout.write(`[OK] ${ids}${msg}\n`);
      }
      break;
    case "step":
      if (useColor) {
        process.stdout.write(`\x1b[2K\r[\x1b[33m>>\x1b[0m] ${ids}${msg}`);
      } else {
        process.stdout.write(`[>>] ${ids}${msg}\n`);
      }
      break;
  }
}

export function banner(): void {
  if (!printBanner) return;

  log(`${_DEFINE_PROG} v${_DEFINE_VER}`);
  process.stdout.write(
    `${"-".repeat(process.stdout.columns < 80 ? process.stdout.columns : 80)}\n`,
  );

  printBanner = false;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
