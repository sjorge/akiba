import { _DEFINE_PROG, _DEFINE_VER } from "vars";
import tty from "node:tty";
import process from "node:process";

let printBanner = true;

export function log(
  msg: string,
  type: "error" | "warn" | "step" | "done" | "info" = "info",
): void {
  const useColor: boolean = tty.isatty(process.stdout.fd);
  switch (type) {
    case "error":
      if (useColor) {
        process.stderr.write(`\x1b[2K\r[\x1b[31m!!\x1b[0m] ${msg}\n`);
      } else {
        process.stdout.write(`[!!] ${msg}\n`);
      }
      break;
    case "warn":
      if (useColor) {
        process.stdout.write(`\x1b[2K\r[\x1b[33mWW\x1b[0m] ${msg}\n`);
      } else {
        process.stdout.write(`[WW] ${msg}\n`);
      }
      break;
    case "info":
      if (useColor) {
        process.stdout.write(`\x1b[2K\r[\x1b[34mII\x1b[0m] ${msg}\n`);
      } else {
        process.stdout.write(`[II] ${msg}\n`);
      }
      break;
    case "done":
      if (useColor) {
        process.stdout.write(`\x1b[2K\r[\x1b[32mOK\x1b[0m] ${msg}\n`);
      } else {
        process.stdout.write(`[OK] ${msg}\n`);
      }
      break;
    case "step":
      if (useColor) {
        process.stdout.write(`\x1b[2K\r[\x1b[33m>>\x1b[0m] ${msg}`);
      } else {
        process.stdout.write(`[>>] ${msg}\n`);
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
