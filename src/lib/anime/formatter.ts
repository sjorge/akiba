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
    this.name = "AnimeFormatStringException";
  }
}

export function animeStringFormat(
  template: string,
  data: AnimeStringFormatData,
): string {
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

  // apply tempalte
  return sanitizeDirComponents(
    template.replace(
      /{(\w+)(?::(\w+))?}/g,
      (match: string, tag: keyof AnimeStringFormatData, modifier?: string) => {
        if (data[tag] === undefined)
          throw new AnimeStringFormatException(
            `The tag ${match} is not known, available tags: ${Object.keys(data).join(", ")}, modifier: upper, lower, lower_first, upper_first, number, or array index as an int.`,
          );
        switch (modifier?.toLowerCase()) {
          case "upper":
            return sanitizeTag(`${data[tag]}`.toUpperCase());
          case "lower":
            return sanitizeTag(`${data[tag]}`.toLowerCase());
          case "upper_first":
            return `${data[tag]}`.toUpperCase().substring(0, 1);
          case "lower_first":
            return `${data[tag]}`.toLowerCase().substring(0, 1);
          case "number": // can be used to turn 013 -> 13
            if (typeof data[tag] === "string") {
              if (data[tag].split("-").length == 2) {
                const tagMultiNumber = data[tag].split("-");
                const tagStartNumber = parseInt(tagMultiNumber[0], 10);
                const tagStopNumber = parseInt(tagMultiNumber[1], 10);
                if (!isNaN(tagStartNumber) && !isNaN(tagStopNumber))
                  return `${tagStartNumber}-${tagStopNumber}`;
              } else {
                const tagNumber = parseInt(data[tag], 10);
                if (!isNaN(tagNumber)) return `${tagNumber}`;
              }
            }
            return sanitizeTag(`${data[tag]}`);
          default:
            if (Array.isArray(data[tag])) {
              const index = modifier ? parseInt(modifier, 10) : 0;
              if (!isNaN(index) && index < data[tag].length) {
                return sanitizeTag(data[tag][index]);
              }
            }
            return sanitizeTag(`${data[tag]}`);
        }
      },
    ),
  );
}

export function animeStringValidate(
  template: string,
  thowException: boolean = false,
): boolean {
  try {
    animeStringFormat(template, animeStringValidatorData);
  } catch (_e: unknown) {
    if (thowException) throw _e;
    return false;
  }

  return true;
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
