/*
 * Field types used to parse the values of AniDB UDP responses
 */
import type { DateflagsType } from "lib/anidb/types";

export type FieldParser<T> = (value: string) => T;

/*
 * Parses a list. AniDB separates the items of a list with a single quote, a few
 * fields (categories, tags, character ids) use a comma. `type`: item parser
 * function. `separator`: separator to use, default is `'`. `filterEmpty`:
 * filter out empty string items.
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
  "end_unknown_month",
  "ended",
  "start_unknown_year",
  "end_unknown_year",
];

/*
 * Date flags mark the unknown parts of a date (spec: dateflags): bit 0 start
 * date with unknown day, bit 1 start date with unknown month (and day), bit 2
 * end date with unknown day, bit 3 end date with unknown month (and day), bit 4
 * the air date is in the past (the anime has ended), bit 5 start date with
 * unknown year, bit 6 end date with unknown year.
 */
export function Dateflags(flags: number | string): DateflagsType[] {
  return VALID_DATE_FLAGS.filter((_v, i) => !!((Number(flags) >>> i) & 1));
}

/*
 * Parses a `|` separated record with a dynamic definition, the order of the
 * definition decides which value goes to which field.
 * `separator`: separator, default is `|`
 */
export function MapField(
  definition: Map<string, FieldParser<unknown>>,
  separator?: string,
): (value: unknown) => Record<string, unknown> {
  return (value) => {
    const fields = String(value).split(separator || "|");
    // additional trailing fields are ignored, the spec says new ones get added
    if (fields.length < definition.size) {
      throw new Error(
        `reply has ${fields.length} fields, ${definition.size} expected`,
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

/*
 * Like MapField, for hardcoded definitions: the return type follows the
 * definition. It relies on the order of the keys of the object: integer like
 * keys (even as string, i.e. "11") are moved to the front, do not use it for
 * those.
 * `separator`: separator, default is `|`
 */
export function MapFieldStatic<V extends Record<string, FieldParser<unknown>>>(
  definition: V,
  separator?: string,
): (value: unknown) => { [K in keyof V]: ReturnType<V[K]> } {
  return (value) => {
    const fields = String(value).split(separator || "|");
    const entries = Object.entries(definition);
    // additional trailing fields are ignored, the spec says new ones get added
    if (fields.length < entries.length) {
      throw new Error(
        `reply has ${fields.length} fields, ${entries.length} expected`,
      );
    }
    const parsed: Record<string, unknown> = {};
    entries.forEach(([fieldName, fieldParser], idx) => {
      parsed[fieldName] = fieldParser(fields[idx]!);
    });
    return parsed as { [K in keyof V]: ReturnType<V[K]> };
  };
}

/*
 * Looks a coded value up in `def`, optionally after converting the key with
 * `parser`. Unknown codes give `undefined`, as in the old client.
 */
export function CodedField<
  T extends number | string,
  U extends number | string,
>(
  def: Record<U, T>,
  parser?: (key: string, ...args: unknown[]) => U,
): (key: string, ...args: unknown[]) => T {
  return (key, ...args) => def[parser ? parser(key, ...args) : (key as U)];
}

/*
 * AniDB sends booleans as 1 or 0.
 */
export function NumBoolean(value: string): boolean {
  return Boolean(Number(value));
}

/*
 * AniDB sends newlines in returned text as `<br />`. NOTE: the spec also says
 * that a single quote is sent as a backtick and `|` as `/`, but replies contain
 * real apostrophes, so that is not undone.
 */
export function StringWithLineBreaks(value: string | number): string {
  return String(value).replaceAll(/<br \/>/gi, "\n");
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
