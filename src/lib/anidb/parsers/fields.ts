/*
 * Field types used to parse the values of AniDB UDP responses
 */
import type { DateflagsType } from "lib/anidb/types";

export type FieldParser<T> = (value: string) => T;

/**
 * @param type item parser function
 * @param separator separator to use, default is `'`
 * @param filterEmpty filter out empty string items
 */
export function ListField<T>(
  type: FieldParser<T>,
  separator?: string,
  filterEmpty?: boolean,
): FieldParser<T[]> {
  return (value) =>
    (value || "")
      .split(separator || "'")
      .filter((v) => (filterEmpty ? v !== "" : true))
      .map((v) => type(v));
}

const VALID_DATE_FLAGS: readonly DateflagsType[] = [
  "start_unknown_day",
  "start_unknown_month",
  "end_unknown_day",
  // TODO: phase 3 misspelled in the old client, kept for the same output
  "end_unkown_month",
  "ended",
  "start_unknown_year",
  "end_unknown_year",
];

export function Dateflags(flags: number | string): DateflagsType[] {
  return VALID_DATE_FLAGS.filter((_v, i) => !!((Number(flags) >>> i) & 1));
}

/**
 * Parses a `|` separated record with a dynamic definition, the order of the
 * definition decides which value goes to which field.
 * @param separator separator, default is `|`
 */
export function MapField(
  definition: Map<string, FieldParser<unknown>>,
  separator?: string,
): (value: unknown) => Record<string, unknown> {
  return (value) => {
    const fields = String(value).split(separator || "|");
    if (fields.length !== definition.size) {
      throw new Error(
        `Definition and input item length missmatch input: ${fields.length} definition: ${definition.size}`,
      );
    }
    const parsed: Record<string, unknown> = {};
    let idx = 0;
    for (const [fieldName, fieldParser] of definition) {
      parsed[fieldName] = fieldParser(fields[idx]!);
      idx++;
    }
    return parsed;
  };
}

/**
 * Like MapField, for hardcoded definitions: the return type follows the
 * definition. It relies on the order of the keys of the object: integer like
 * keys (even as string, i.e. "11") are moved to the front, do not use it for
 * those.
 * @param separator separator, default is `|`
 */
export function MapFieldStatic<V extends Record<string, FieldParser<unknown>>>(
  definition: V,
  separator?: string,
): (value: unknown) => { [K in keyof V]: ReturnType<V[K]> } {
  return (value) => {
    const fields = String(value).split(separator || "|");
    const entries = Object.entries(definition);
    if (fields.length !== entries.length) {
      throw new Error(
        `Definition and input item length missmatch input: ${fields.length} definition: ${entries.length}`,
      );
    }
    const parsed: Record<string, unknown> = {};
    entries.forEach(([fieldName, fieldParser], idx) => {
      parsed[fieldName] = fieldParser(fields[idx]!);
    });
    return parsed as { [K in keyof V]: ReturnType<V[K]> };
  };
}

/**
 * Looks a coded value up in `def`, optionally after converting the key with
 * `parser`. Unknown codes give `undefined`, as in the old client.
 */
export function CodedField<T extends number | string, U extends number | string>(
  def: Record<U, T>,
  parser?: (key: string, ...args: unknown[]) => U,
): (key: string, ...args: unknown[]) => T {
  return (key, ...args) => def[parser ? parser(key, ...args) : (key as U)];
}

export function NumBoolean(value: string): boolean {
  return Boolean(Number(value));
}

export function StringWithLineBreaks(value: string | number): string {
  return String(value).replaceAll(/<br \/>/gi, "\n");
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
