/*
 * Strict number parsing for user input and stored text
 */

const WHOLE_INTEGER = /^\s*[+-]?\d+\s*$/;

/*
 * Parse `value` as one whole integer: surrounding whitespace, an optional
 * sign, then ASCII digits. Anything else gives undefined, so junk after the
 * digits, decimals, exponents and hex are never silently truncated. Integers
 * beyond Number.MAX_SAFE_INTEGER are rejected too, since they cannot be
 * represented exactly.
 */
export function parseWholeInteger(value: string): number | undefined {
  if (!WHOLE_INTEGER.test(value)) {
    return undefined;
  }
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : undefined;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
