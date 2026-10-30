/*
 * Parser for responses with a fixed, `|` separated field list
 */
import type { FieldParser } from "lib/anidb/parsers/fields";

import { MapFieldStatic } from "lib/anidb/parsers/fields";

export class SimpleParser<V extends Record<string, FieldParser<unknown>>> {
  public parseData: (data: string) => { [K in keyof V]: ReturnType<V[K]> };

  public constructor(responseDefinition: V) {
    this.parseData = MapFieldStatic(responseDefinition, "|");
  }

  public toString(): string {
    return "SimpleParser";
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
