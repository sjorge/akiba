/*
 * Parser for responses with a fixed, `|` separated field list
 */
import { MapFieldStatic } from "lib/anidb/parsers/fields";
import type { FieldParser } from "lib/anidb/parsers/fields";

export class SimpleParser<V extends Record<string, FieldParser<unknown>>> {
  parseData: (data: string) => { [K in keyof V]: ReturnType<V[K]> };

  constructor(responseDefinition: V) {
    this.parseData = MapFieldStatic(responseDefinition, "|");
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
