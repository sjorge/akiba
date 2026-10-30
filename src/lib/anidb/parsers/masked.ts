/*
 * Parser for responses where the requested fields are selected with a bit mask
 */
import type { FieldParser } from "lib/anidb/parsers/fields";

import { MapField } from "lib/anidb/parsers/fields";

// field name to `[byte, bit value, parser]`
export type MaskDefinition = Record<
  string,
  readonly [number, number, FieldParser<unknown>]
>;

export type MaskResult<D extends MaskDefinition> = {
  -readonly [K in keyof D]: ReturnType<D[K][2]>;
};

/*
 * FILE and ANIME replies only hold the fields that were asked for with a mask:
 * hexadecimal bytes in which every bit selects one field. Fields come back in
 * table order (byte 1 bit 7 first, down to bit 0, then byte 2 and so on), so
 * the order of the mask decides which value is which field. Retired, unused and
 * reserved bits are not in the definitions because the server answers 505 when
 * they are set.
 */
export class MaskedParser<D extends MaskDefinition> {
  // requested fields, sorted by position in the mask
  public fields: string[];
  // the mask as hex string
  public mask: string;
  public parseData: (data: string) => Partial<MaskResult<D>>;

  /*
   * `mask`: list of field names
   */
  public constructor(
    protected definition: D,
    mask: Array<keyof D & string>,
  ) {
    this.fields = this.prepareMaskArray(mask);
    this.mask = this.encode(this.fields);

    this.parseData = MapField(
      new Map(this.fields.map((field) => [field, this.definition[field]![2]])),
    ) as (data: string) => Partial<MaskResult<D>>;
  }

  public toString(): string {
    return "MaskedParser";
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
