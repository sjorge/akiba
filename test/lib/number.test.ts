import { expect, test } from "bun:test";

import { parseWholeInteger } from "lib/number";

test("parseWholeInteger accepts a whole integer with optional sign and padding", (): void => {
  expect(parseWholeInteger("12")).toBe(12);
  expect(parseWholeInteger("+12")).toBe(12);
  expect(parseWholeInteger(" 12 ")).toBe(12);
  expect(parseWholeInteger("-1")).toBe(-1);
  expect(parseWholeInteger("0")).toBe(0);
});

test("parseWholeInteger rejects anything that is not one whole integer", (): void => {
  for (const bad of [
    "12x",
    "1.5",
    "1e3",
    "0x10",
    "",
    "abc",
    "9007199254740993",
  ]) {
    expect(parseWholeInteger(bad), JSON.stringify(bad)).toBeUndefined();
  }
});
