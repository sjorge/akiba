import tty from "node:tty";
import { expect, spyOn, test } from "bun:test";

import { banner, log } from "lib/logger";
import { captureStdout } from "test/helpers";

test("logger prints plain output without a tty", async (): Promise<void> => {
  const isatty = spyOn(tty, "isatty").mockReturnValue(false);
  const columns = Object.getOwnPropertyDescriptor(process.stdout, "columns");
  Object.defineProperty(process.stdout, "columns", {
    value: undefined,
    configurable: true,
  });

  try {
    // NOTE: regression for a missing rule when columns is undefined, and the
    //       banner is only printed once
    const first = await captureStdout((): void => banner());
    expect(first).toMatch(/^\[II\] akiba v\d+\.\d+\.\d+\n-{80}\n$/);
    expect(await captureStdout((): void => banner())).toBe("");

    const out = await captureStdout((): void => {
      log("message", "error");
      log("message", "warn");
      log("message", "info");
      log("message");
      log("message", "done");
      log("message", "step");
      log("message", "info", true, { anidb: 12, anilist: 3 });
      log("message", "info", true);
    });
    expect(out).toBe(
      [
        "[!!] message",
        "[WW] message",
        "[II] message",
        "[II] message",
        "[OK] message",
        "[>>] message",
        "[II] {## |    12} message",
        "[II] {   |      } message",
        "",
      ].join("\n"),
    );
  } finally {
    isatty.mockRestore();
    if (columns) Object.defineProperty(process.stdout, "columns", columns);
    else delete (process.stdout as { columns?: number }).columns;
  }
});

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
