/*
 * Parser for FILE responses: bytes 0 to 4 of the mask are the fmask, 5 to 8 the
 * amask
 */
import type { MaskDefinition, MaskResult } from "lib/anidb/parsers/masked";
import type { FileResult } from "lib/anidb/types";

import { ListField, NumBoolean } from "lib/anidb/parsers/fields";
import { MaskedParser } from "lib/anidb/parsers/masked";

const amaskDef = {
  /*
  7 128 unused
  6 64 int4 aid
  5 32 int4 eid
  4 16 int4 gid
  3 8 int4 mylist id
  2 4 list other episodes
  1 2 int2 IsDeprecated
  0 1 int2 state
  */
  aid: [0, 64, Number],
  eid: [0, 32, Number],
  gid: [0, 16, Number],
  mylist_id: [0, 8, Number],
  // NOTE: the layout of other episodes (episode id and percentage) is not
  // specified, so it stays a raw string
  other_episodes: [0, 4, String],
  is_deprecated: [0, 2, NumBoolean],
  state: [0, 1, Number],
  /*
  7 128 int8 size
  6 64 str ed2k
  5 32 str md5
  4 16 str sha1
  3 8 str crc32
  2 4 unused
  1 2 video colour depth
  0 1 reserved
  */
  size: [1, 128, Number],
  ed2k: [1, 64, String],
  md5: [1, 32, String],
  sha1: [1, 16, String],
  crc32: [1, 8, String],
  video_color_depth: [1, 2, String],
  /*
  7 128 str quality
  6 64 str source
  5 32 str audio codec list
  4 16 int4 audio bitrate list
  3 8 str video codec
  2 4 int4 video bitrate
  1 2 str video resolution
  0 1 str file type (extension)
  */
  quality: [2, 128, String],
  source: [2, 64, String],
  audio_codecs: [2, 32, ListField(String, undefined, true)],
  audio_bitrates: [2, 16, ListField(Number, undefined, true)],
  video_codec: [2, 8, String],
  video_bitrate: [2, 4, Number],
  video_resolution: [2, 2, String],
  file_extension: [2, 1, String],
  /*
  7 128 str dub language
  6 64 str sub language
  5 32 int4 length in seconds
  4 16 str description
  3 8 int4 aired date
  2 4 unused
  1 2 unused
  0 1 str anidb file name
  */
  dub_language: [3, 128, ListField(String, undefined, true)],
  sub_language: [3, 64, ListField(String, undefined, true)],
  length_seconds: [3, 32, Number],
  description: [3, 16, String],
  aired_date: [3, 8, Number],
  anidb_filename: [3, 1, String],
  /*
  7 128 int4 mylist state
  6 64 int4 mylist filestate
  5 32 int4 mylist viewed
  4 16 int4 mylist viewdate
  3 8 str mylist storage
  2 4 str mylist source
  1 2 str mylist other
  0 1 unused
  */
  mylist_state: [4, 128, Number],
  mylist_filestate: [4, 64, Number],
  mylist_viewed: [4, 32, Number],
  mylist_viewdate: [4, 16, Number],
  mylist_storage: [4, 8, String],
  mylist_source: [4, 4, String],
  mylist_other: [4, 2, String],
  /*
  7 128 int4 anime total episodes
  6 64 int4 highest episode number
  5 32 str year
  4 16 str type
  3 8 str related aid list
  2 4 str related aid type
  1 2 str category list
  0 1 reserved
  */
  anime_total_episodes: [5, 128, Number],
  highest_episode: [5, 64, Number],
  year: [5, 32, String],
  type: [5, 16, String],
  related_aid_list: [5, 8, ListField(Number, undefined, true)],
  related_aid_type: [5, 4, ListField(Number, undefined, true)],
  category_list: [5, 2, ListField(String, ",", true)],
  /*
  7 128 str romaji name
  6 64 str kanji name
  5 32 str english name
  4 16 str other name
  3 8 str short name list
  2 4 str synonym list
  1 2 retired
  0 1 retired
  */
  romanji_name: [6, 128, String],
  kanji_name: [6, 64, String],
  english_name: [6, 32, String],
  other_name_list: [6, 16, ListField(String, undefined, true)],
  short_name_list: [6, 8, ListField(String, undefined, true)],
  synonym_list: [6, 4, ListField(String, undefined, true)],
  /*
  7 128 str epno
  6 64 str ep name
  5 32 str ep romaji name
  4 16 str ep kanji name
  3 8 int4 episode rating
  2 4 int4 episode vote count
  1 2 unused
  0 1 unused
  */
  epno: [7, 128, String],
  ep_name: [7, 64, String],
  ep_romanji_name: [7, 32, String],
  ep_kanji_name: [7, 16, String],
  ep_rating: [7, 8, Number],
  ep_vote_count: [7, 4, Number],
  /*
  7 128 str group name
  6 64 str group short name
  5 32 unused
  4 16 unused
  3 8 unused
  2 4 unused
  1 2 unused
  0 1 int4 date aid record updated
  */
  group_name: [8, 128, String],
  group_short_name: [8, 64, String],
  aid_record_updated: [8, 1, Number],
} as const satisfies MaskDefinition;

export type FileParseResult = Partial<MaskResult<typeof amaskDef>> & {
  fid: number;
};

/*
 * Parser for FILE replies. The mask is nine bytes: the first five are the fmask
 * (fields of the file) and the last four the amask (fields of the anime,
 * episode and group), as in the FILE command (`fmask=` and `amask=`). `fid`
 * always comes first in the reply and is not part of the mask.
 */
export class FileParser extends MaskedParser<typeof amaskDef> {
  public static readonly mask_definition = amaskDef;

  public constructor(fields: Array<keyof FileResult & string>) {
    super(amaskDef, fields as Array<keyof typeof amaskDef>);

    const origParseData = this.parseData;
    this.parseData = (data: string): FileParseResult => {
      // NOTE: several matching files (322) are handled by the client
      const matchedData = data.match(/([0-9]+)\|((:?.|\n)*)/);
      if (!matchedData) {
        throw new Error("Bad data: " + data);
      }
      const respObj = origParseData(matchedData[2]!);
      return { ...respObj, fid: Number(matchedData[1]) };
    };
  }

  public toString(): string {
    return "FileParser";
  }

  public getFmask(): string {
    return this.mask.slice(0, 5 * 2);
  }

  public getAmask(): string {
    return this.mask.slice(5 * 2);
  }
}

// compile time check: the mask definition describes exactly FileResult
type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
export const fileResultMatches: Same<
  MaskResult<typeof amaskDef> & { fid: number },
  FileResult
> = true;

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
