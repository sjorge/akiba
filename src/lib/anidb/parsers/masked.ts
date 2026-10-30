/*
 * Parser for responses where the requested fields are selected with a bit mask
 */
import { MapField } from "lib/anidb/parsers/fields";
import type { FieldParser } from "lib/anidb/parsers/fields";

/** field name to `[byte, bit value, parser]` */
export type MaskDefinition = Record<
  string,
  readonly [number, number, FieldParser<unknown>]
>;

export type MaskResult<D extends MaskDefinition> = {
  -readonly [K in keyof D]: ReturnType<D[K][2]>;
};

export class MaskedParser<D extends MaskDefinition> {
  /** requested fields, sorted by position in the mask */
  fields: string[];
  /** the mask as hex string */
  mask: string;
  parseData: (data: string) => Partial<MaskResult<D>>;

  /** @param mask list of field names */
  constructor(
    protected definition: D,
    mask: Array<keyof D & string>,
  ) {
    this.fields = this.prepareMaskArray(mask);
    this.mask = this.encode(this.fields);

    this.parseData = MapField(
      new Map(this.fields.map((field) => [field, this.definition[field]![2]])),
    ) as (data: string) => Partial<MaskResult<D>>;
  }

  private prepareMaskArray(maskArray: string[]): string[] {
    const deduplicated = Array.from(new Set(maskArray));

    // check all exist!
    const incorrectField = deduplicated.find(
      (i) => !Object.hasOwn(this.definition, i),
    );
    if (incorrectField !== undefined) {
      throw new Error(`Bad field: ${incorrectField}`);
    }

    const position = (field: string): number =>
      this.definition[field]![0] * 1000 - this.definition[field]![1];
    return deduplicated.sort((a, b) => position(a) - position(b));
  }

  private encode(maskArray: string[]): string {
    const bytes: number[] = Array(
      Object.values(this.definition).reduce(
        (m, c) => (c[0] + 1 > m ? c[0] + 1 : m),
        0,
      ),
    ).fill(0);
    for (const maskItem of maskArray) {
      const [byteNum, bitValue] = this.definition[maskItem]!;
      bytes[byteNum] = (bytes[byteNum] || 0) + bitValue;
    }
    return Buffer.from(bytes).toString("hex");
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
