/*
 * Error type thrown by the AniDB UDP client
 */
import type { ResponseCode, ResponseLabel } from "lib/anidb/constants";

import { RESPONSE_CODE, getResponse } from "lib/anidb/constants";

const KNOWN_CODES: ReadonlySet<number> = new Set(Object.values(RESPONSE_CODE));

/*
 * Thrown for replies the client treats as failures: every code above 305 that
 * the command did not expect, plus the client side codes 670 to 673 (unknown
 * error, timeout, not connected, failed to initialize). `code` and `label` are
 * the return code and its name from the spec. `message` is the text of the
 * server (the reason for a ban) or the code and name. `payload` holds parsed
 * data for some errors: the current entry for 310, the file ids for 322 and the
 * vote for 360, 363 and 364.
 */
export class AnidbError extends Error {
  public code: ResponseCode;
  public label: ResponseLabel;
  // parsed anidb response (some errors carry payloads, usually specified on the
  // method docs)
  public payload: Record<string | number, unknown> | undefined;

  /*
   * `code`: response code, unknown or missing codes become UNKNOWN_API_ERROR
   * `message`: additional message
   */
  public constructor(
    code?: ResponseCode,
    message?: string,
    payload?: Record<string | number, unknown>,
  ) {
    super();
    const fixedCode: ResponseCode =
      code !== undefined && KNOWN_CODES.has(code)
        ? code
        : RESPONSE_CODE.UNKNOWN_API_ERROR;
    const label: ResponseLabel = getResponse(fixedCode) ?? "UNKNOWN_API_ERROR";

    this.name = "AnidbError";
    this.message = message || `${fixedCode}: ${label}`;
    this.code = fixedCode;
    this.label = label;
    this.payload = payload;
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
