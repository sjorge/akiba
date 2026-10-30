/*
 * Response parsers: masked file/anime data and simple field maps
 */
export {
  CodedField,
  Dateflags,
  ListField,
  MapField,
  MapFieldStatic,
  NumBoolean,
  StringWithLineBreaks,
} from "lib/anidb/parsers/fields";
export type { FieldParser } from "lib/anidb/parsers/fields";
export { MaskedParser } from "lib/anidb/parsers/masked";
export type { MaskDefinition, MaskResult } from "lib/anidb/parsers/masked";
export { SimpleParser } from "lib/anidb/parsers/simple";
export { FileParser } from "lib/anidb/parsers/file";
export type { FileParseResult } from "lib/anidb/parsers/file";
export { AnimeParser } from "lib/anidb/parsers/anime";

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
