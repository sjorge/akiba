/*
 * Error type thrown by the AniDB UDP client
 */
import { RESPONSE_CODE, getResponse } from "lib/anidb/constants";
import type { ResponseCode, ResponseLabel } from "lib/anidb/constants";

const KNOWN_CODES: ReadonlySet<number> = new Set(Object.values(RESPONSE_CODE));

export class AnidbError extends Error {
  code: ResponseCode;
  label: ResponseLabel;
  /** Parsed anidb response (some errors carry payloads, usually specified on the method docs) */
  payload: Record<string | number, unknown> | undefined;

  /**
   * @param code response code, unknown or missing codes become UNKNOWN_API_ERROR
   * @param message additional message
   */
  constructor(
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
