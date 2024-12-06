/*
 * Internal helper types and function for anidb
 */
import type { Config } from "lib/config";

export class AnimeFormatStringException extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AnimeFormatStringException";
  }
}

export type AnimeFormatStringData = {
  fid: string;
  aid: string;
  eid: string;
  gid: string;
  lid: string;
  status: string;
  size: string;
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
  length: string;
  description: string;
  group: string;
  group_short: string;
  episode: string;
  episode_name: string;
  episode_name_romaji: string;
  episode_name_kanji: string;
  episode_total: string;
  episode_last: string;
  anime_year: string;
  anime_type: string;
  anime_name_romaji: string;
  anime_name_kanji: string;
  anime_name_english: string;
  anime_name_other: string;
  anime_name_short: string;
  anime_synonyms: string;
  anime_category: string;
  version: string;
  censored: string;
  orginal_name: string;
};

export function animeFormatString(
  template: string,
  data: AnimeFormatStringData,
): string {
  return template.replace(
    /{(\w+)}/g,
    (match: string, tag: keyof AnimeFormatStringData) => {
      if (data[tag] === undefined)
        throw new AnimeFormatStringException(
          `The tag ${match} is not known, available: ${Object.keys(data).join(", ")}.`,
        );

      return data[tag];
    },
  );
}

export function animeFormatValidate(
  template: string,
  thowException: boolean = false,
): boolean {
  try {
    animeFormatString(template, {
      fid: "",
      aid: "",
      eid: "",
      gid: "",
      lid: "",
      status: "",
      size: "",
      ed2k: "",
      md5: "",
      sha1: "",
      crc32: "",
      lang_dub: "",
      lang_sub: "",
      quaility: "",
      source: "",
      audio_codec: "",
      audio_bitrate: "",
      video_codec: "",
      video_bitrate: "",
      resolution: "",
      filetype: "",
      length: "",
      description: "",
      group: "",
      group_short: "",
      episode: "",
      episode_name: "",
      episode_name_romaji: "",
      episode_name_kanji: "",
      episode_total: "",
      episode_last: "",
      anime_year: "",
      anime_type: "",
      anime_name_romaji: "",
      anime_name_kanji: "",
      anime_name_english: "",
      anime_name_other: "",
      anime_name_short: "",
      anime_synonyms: "",
      anime_category: "",
      version: "",
      censored: "",
      orginal_name: "",
    });
  } catch (_e: unknown) {
    if (thowException) throw _e;
    return false;
  }

  return true;
}

export class AnimeRenamer {
  private format: string;
  private rehash: boolean;

  public constructor(config: Config, rehash: boolean = false, format?: string) {
    this.format = config.renamer.format;
    this.rehash = rehash;
    if (format) this.format = format;
  }

  public toString(): string {
    return "AnimeRenamer";
  }

  public identify(animeEpisodeFile: string): void {
    console.log(`TODO identify ${animeEpisodeFile}`);
  }

  public rename(
    animeEpisodeFile: string,
    metadata: AniDB_Show,
    overwrite: boolean = false,
    symlink: boolean = true,
  ): void {
    console.log(
      `TODO ${symlink ? "symlink" : "move"} ${animeEpisodeFile} using metadata ${metadata}, overwrite = ${overwrite}`,
    );
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
