/*
 * Request encoding and response parsing for the AniDB UDP API
 */
import crypto from "node:crypto";

import type { ParsedResponse } from "lib/anidb/types";

const TAG_ALPHABET: string =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const TAG_LENGTH: number = 5;

/*
 * Tags (`tag=`) are echoed by the server in front of every reply line, that is
 * how a reply is matched to its request. A tag must not start with two zero
 * bytes, which mark compressed datagrams; letters and digits never do.
 */
export function makeTag(): string {
  let tag = "";
  for (let i = 0; i < TAG_LENGTH; i++) {
    tag += TAG_ALPHABET.charAt(crypto.randomInt(TAG_ALPHABET.length));
  }
  return tag;
}

/*
 * A request is one datagram with one line: `COMMAND key=value&key=value`.
 * Values are escaped like an html form: `&` is sent as `&amp;` and newlines as
 * `<br />` (spec: content encoding).
 */
export function encodeRequest(
  command: string,
  params: Record<string, string | number>,
): string {
  const paramsString = Object.entries(params)
    .map(([key, value]) => {
      const escapedValue = String(value)
        .replaceAll(/\r\n|\r|\n/gi, "<br />")
        .replaceAll(/&/gi, "&amp;");
      return `${key}=${escapedValue}`;
    })
    .join("&");
  return command + " " + paramsString;
}

/*
 * A reply is `{3 digit code} {text}` followed by data lines, lines end with
 * `\n` (never `\r\n`) and the fields of a line are separated by `|`. Some codes
 * (200, 201, 209, 271, 272, 504) carry data on the first line, which is why
 * `response` holds the words of that line.
 */
export function parseResponse(resp: string): ParsedResponse {
  const matched = resp.match(/(^[0-9]{3})\s(.+)(?:\n((?:.|\n)+))?/m);
  if (!matched) throw new Error("BAD_RESPONSE");
  return {
    code: Number(matched[1]),
    response: matched[2]!.trim().split(" "),
    data: (matched[3] ?? "").trim(),
  };
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
