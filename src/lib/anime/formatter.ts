/*
 * Internal helper types and function for formatting strings
 */
import path from "node:path";

const animeStringValidatorData = JSON.parse(
  '{"fid":0,"aid":0,"eid":0,"gid":0,"lid":0,"status":0,"size":0,"ed2k":"","md5":"","sha1":"","crc32":"","lang_dub":"","lang_sub":"","quaility":"","source":"","audio_codec":"","audio_bitrate":"","video_codec":"","video_bitrate":"","resolution":"","filetype":"","length":0,"description":"","group":"","group_short":"","episode":"","episode_name":"","episode_name_romaji":"","episode_name_kanji":"","episode_total":0,"episode_last":0,"anime_year":[],"anime_type":"","anime_name_romaji":"","anime_name_kanji":"","anime_name_english":"","anime_name_other":[],"anime_name_short":[],"anime_synonyms":[],"anime_category":[],"version":"","censored":"","orginal_name":""}',
) as AnimeStringFormatData;

export type AnimeStringFormatData = {
  fid: number;
  aid: number;
  eid: number;
  gid: number;
  lid: number;
  status: number;
  size: number;
  ed2k: string;
  md5: string;
  sha1: string;
  crc32: string;
  lang_dub: string;
  lang_sub: string;
  quaility: string;
  source: string;
  audio_codec: string;
  audio_bitrate: string;
  video_codec: string;
  video_bitrate: string;
  resolution: string;
  filetype: string;
  length: number;
  description: string;
  group: string;
  group_short: string;
  episode: string;
  episode_name: string;
  episode_name_romaji: string;
  episode_name_kanji: string;
  episode_total: number;
  episode_last: number;
  anime_year: number[];
  anime_type: string;
  anime_name_romaji: string;
  anime_name_kanji: string;
  anime_name_english: string;
  anime_name_other: string[];
  anime_name_short: string[];
  anime_synonyms: string[];
  anime_category: string[];
  version: string;
  censored: string;
  orginal_name: string;
};

export class AnimeStringFormatException extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AnimeStringFormatException";
  }
}

function sanitizeTag(template: string): string {
  return template
    .replace(/[`]/g, "'")
    .replace(/\//g, "")
    .replace(/[^a-zA-Z0-9-&!`',.~+\- ()]/g, "_")
    .replace(/[_]+/g, "_")
    .replace(/^_|_$/g, "")
    .replace(/_\./g, ".")
    .replace(/_!/g, "!")
    .replace(/_-\./g, "-")
    .replace(/_\s/g, " ")
    .replace(/\s_/g, " ");
}

// NOTE: mostly because CIFS has issues with directories ending in "."
function sanitizeDirComponents(template: string): string {
  const templateNew: string[] = [];
  for (let chunk of template.split(path.sep)) {
    if (chunk.endsWith(".") && chunk !== path.basename(template)) {
      chunk = chunk.substring(0, chunk.length - 1);
    }
    templateNew.push(chunk);
  }
  return templateNew.join(path.sep);
}

export function animeStringFormat(
  template: string,
  data: AnimeStringFormatData,
): string {
  // parse first, so unknown tags are reported before any rendering error
  const parts = parseTemplate(template);
  let rendered = "";
  for (const part of parts) {
    if (typeof part === "string") rendered += part;
    else rendered += renderTag(part, data);
  }
  return sanitizeDirComponents(rendered);
}

type TemplateTag = {
  match: string;
  tag: keyof AnimeStringFormatData;
  modifier?: string;
};

const keywordModifiers = new Set([
  "upper",
  "lower",
  "upper_first",
  "lower_first",
  "number",
]);

function parseTemplate(template: string): (string | TemplateTag)[] {
  const parts: (string | TemplateTag)[] = [];
  let last = 0;
  for (const m of template.matchAll(/{(\w+)(?::(\w+))?}/g)) {
    // NOTE: own properties only, so {constructor} or {toString} are unknown
    if (!Object.hasOwn(animeStringValidatorData, m[1]))
      throw new AnimeStringFormatException(
        `The tag ${m[0]} is not known, available tags: ${Object.keys(animeStringValidatorData).join(", ")}, modifier: upper, lower, lower_first, upper_first, number, or array index as an int.`,
      );
    parts.push(template.substring(last, m.index));
    parts.push({
      match: m[0],
      tag: m[1] as keyof AnimeStringFormatData,
      modifier: m[2],
    });
    last = m.index + m[0].length;
  }
  parts.push(template.substring(last));
  return parts;
}

function numberElementError(match: string): AnimeStringFormatException {
  return new AnimeStringFormatException(
    `The tag ${match} selects a number from a list, which cannot be used as text.`,
  );
}

// the array index a modifier names, the whole modifier must be digits
function arrayIndex(modifier?: string): number | undefined {
  if (modifier === undefined) return 0;
  if (!/^\d+$/.test(modifier)) return undefined;
  const index = Number(modifier);
  return Number.isSafeInteger(index) ? index : undefined;
}

// sanitised, as written unless the whole value is a number or a range of them
function numberModifier(value: unknown): string {
  if (typeof value === "string") {
    const stripZeros = (digits: string): string =>
      digits.replace(/^0+(?=\d)/, "");
    if (/^\d+$/.test(value)) return stripZeros(value);
    if (/^\d+-\d+$/.test(value)) {
      const [start, stop] = value.split("-");
      return `${stripZeros(start)}-${stripZeros(stop)}`;
    }
  }
  return sanitizeTag(`${value}`);
}

function renderTag(
  { match, tag, modifier }: TemplateTag,
  data: AnimeStringFormatData,
): string {
  const value: unknown = data[tag];
  const modifierLower = modifier?.toLowerCase();
  switch (modifierLower) {
    case "upper":
      return sanitizeTag(`${value}`.toUpperCase());
    case "lower":
      return sanitizeTag(`${value}`.toLowerCase());
    case "upper_first":
      return sanitizeTag(`${value}`.toUpperCase().substring(0, 1));
    case "lower_first":
      return sanitizeTag(`${value}`.toLowerCase().substring(0, 1));
    case "number": // can be used to turn 013 -> 13
      return numberModifier(value);
    default:
      if (Array.isArray(value)) {
        const index = arrayIndex(modifier);
        if (index !== undefined && index < value.length) {
          if (typeof value[index] !== "string") throw numberElementError(match);
          return sanitizeTag(value[index]);
        }
      }
      return sanitizeTag(`${value}`);
  }
}

export function animeStringValidate(
  template: string,
  throwException: boolean = false,
): boolean {
  try {
    // NOTE: a number picked from a list only fails once the list is not
    // empty, so reject it here whatever the sample holds
    for (const part of parseTemplate(template)) {
      if (
        typeof part !== "string" &&
        part.tag === "anime_year" &&
        !keywordModifiers.has(part.modifier?.toLowerCase() ?? "") &&
        arrayIndex(part.modifier) !== undefined
      )
        throw numberElementError(part.match);
    }
    animeStringFormat(template, animeStringValidatorData);
  } catch (_e: unknown) {
    if (throwException) throw _e;
    return false;
  }

  return true;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
