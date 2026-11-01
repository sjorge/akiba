/*
 * Parsers for the responses of the individual AniDB UDP commands
 */
import type { MylistByAnimeResult } from "lib/anidb/types";

import {
  CodedField,
  Dateflags,
  Int,
  ListField,
  MapFieldStatic,
  NumBoolean,
  SimpleParser,
  StringWithLineBreaks,
  badData,
  parseField,
} from "lib/anidb/parsers";
import { MYLIST_STATES, VOTE_TYPES } from "lib/anidb/constants";
import { reverseObj } from "lib/anidb/util";

/*
 * EPISODE, reply 240:
 * `eid|aid|length|rating|votes|epno|eng|romaji|kanji|aired|type`. The length is
 * in minutes, `type` the raw episode type (1 regular, 2 special S, 3 credits C,
 * 4 trailer T, 5 parody P, 6 other O).
 */
export const EPISODE_PARSER = new SimpleParser({
  eid: Int,
  aid: Int,
  length: Int,
  rating: Int,
  votes: Int,
  epno: StringWithLineBreaks,
  eng: StringWithLineBreaks,
  romaji: StringWithLineBreaks,
  kanji: StringWithLineBreaks,
  aired: Int,
  type: CodedField({
    "1": "",
    "2": "S",
    "3": "C",
    "4": "T",
    "5": "P",
    "6": "O",
  } as const),
});

/*
 * MYLIST, reply 221 (also the record that comes with 310 FILE ALREADY IN
 * MYLIST):
 * `lid|fid|eid|aid|gid|date|state|viewdate|storage|source|other|filestate`.
 * `other` is the only field with newlines. File states: 0 normal/original, 1
 * corrupted/invalid crc, 2 self edited, 10 self ripped, 11 on dvd, 12 on vhs,
 * 13 on tv, 14 in theaters, 15 streamed, 100 other.
 */
export const MYLIST_PARSER = new SimpleParser({
  lid: Int,
  fid: Int,
  eid: Int,
  aid: Int,
  gid: Int,
  date: Int,
  state: CodedField(MYLIST_STATES),
  viewdate: Int,
  storage: StringWithLineBreaks,
  source: StringWithLineBreaks,
  other: StringWithLineBreaks,
  filestate: CodedField({
    "0": "normal/original",
    "1": "corrupted version/invalid crc",
    "2": "self edited",
    "10": "self ripped",
    "11": "on dvd",
    "12": "on vhs",
    "13": "on tv",
    "14": "in theaters",
    "15": "streamed",
    "100": "other",
  } as const),
});

/*
 * ANIMEDESC, reply 233: `current part|max parts|description`. Parts are zero
 * based.
 */
export const ANIME_DESC_PARSER = {
  parseData(data: string): {
    current_part: number;
    max_parts: number;
    desc: string;
  } {
    // the text is the rest of the reply and may hold `|`
    const first = data.indexOf("|");
    const second = first < 0 ? -1 : data.indexOf("|", first + 1);
    if (second < 0) {
      throw badData(`reply has ${data.split("|").length} fields, 3 expected`);
    }
    return {
      current_part: parseField("current_part", Int, data.slice(0, first)),
      max_parts: parseField("max_parts", Int, data.slice(first + 1, second)),
      // raw: a `<br />` can be cut in two by the part size, the client
      // decodes the joined parts
      desc: data.slice(second + 1),
    };
  },
};

/*
 * CALENDAR, reply 297: one `aid|start date|date flags` line per title.
 */
export const CALENDAR_PARSER = ListField(
  MapFieldStatic({
    aid: Int,
    startdate: Int,
    dateflags: Dateflags,
  }),
  "\n",
  true,
);

/*
 * CHARACTER, reply 235: `charid|name kanji|name transcription|pic|anime
 * blocks|episode list|last update|type|gender`. An anime block is
 * `aid,appearance,creatorid,is_main_seiyuu`, blocks are separated by a single
 * quote. Appearance: 0 appears in, 1 cameo appearance in, 2 main character in,
 * 3 secondary cast in.
 */
export const CHARACTER_PARSER = new SimpleParser({
  charid: Int,
  character_name_kanji: StringWithLineBreaks,
  character_name_transc: StringWithLineBreaks,
  pic: String,
  anime_blocks: ListField(
    MapFieldStatic(
      {
        aid: Int,
        appearance: Int,
        creatorid: Int,
        is_main_seiyuu: NumBoolean,
      },
      ",",
    ),
    "'",
    true,
  ),
  episode_list: ListField(Int, ",", true),
  last_update_date: Int,
  type: Int,
  gender: String,
});

/*
 * CREATOR, reply 245: `creatorid|name kanji|name transcription|type|pic|url
 * english|url japanese|wiki url english|wiki url japanese|last update`. Type: 1
 * person, 2 company, 3 collaboration.
 */
export const CREATOR_PARSER = new SimpleParser({
  creatorid: Int,
  creator_name_kanji: StringWithLineBreaks,
  creator_name_transcription: StringWithLineBreaks,
  type: CodedField({
    "1": "person",
    "2": "company",
    "3": "collaboration",
  } as const),
  pic_name: String,
  url_english: String,
  url_japanese: String,
  wiki_url_english: String,
  wiki_url_japanese: String,
  last_update_date: Int,
});

/*
 * GROUP, reply 250: `gid|rating|votes|acount|fcount|name|short|irc channel|irc
 * server|url|picname|founded|disbanded|dateflags|last release|last
 * activity|group relations`. A relation is `othergroupid,relationtype` (1
 * participant in, 2 parent of, 4 merged from, 5 now known as, 6 other),
 * relations are separated by a single quote. The last release date is when a
 * file was released by the group, the last activity when a file was added to
 * AniDB.
 */
const GROUP_RECORD = new SimpleParser({
  gid: Int,
  rating: Int,
  votes: Int,
  acount: Int,
  fcount: Int,
  name: StringWithLineBreaks,
  short: StringWithLineBreaks,
  irc_channel: String,
  irc_server: String,
  url: String,
  picname: String,
  foundeddate: Int,
  disbandeddate: Int,
  dateflags: Dateflags,
  lastreleasedate: Int,
  lastactivitydate: Int,
  grouprelations: ListField(
    MapFieldStatic(
      {
        othergroupid: Int,
        relationtype: CodedField({
          "1": "participant_in",
          "2": "parent_of",
          "4": "merged_from",
          "5": "now_known_as",
          "6": "other",
          "105": "formerly",
        } as const),
      },
      ",",
    ),
    "'",
    true,
  ),
});

// the 16 fields before the relations: a group without relations may come
// without the last field
const GROUP_FIELDS_BEFORE_RELATIONS = 16;

export const GROUP_PARSER = {
  parseData(data: string): ReturnType<typeof GROUP_RECORD.parseData> {
    const count = data.split("|").length;
    return GROUP_RECORD.parseData(
      count === GROUP_FIELDS_BEFORE_RELATIONS ? `${data}|` : data,
    );
  },
};

/*
 * Completion states of a group for an anime (spec: GROUPSTATUS state).
 */
export const GROUP_STATUS_CODES = {
  "1": "ongoing",
  "2": "stalled",
  "3": "complete",
  "4": "dropped",
  "5": "finished",
  "6": "specials_only",
} as const;

/*
 * GROUPSTATUS, reply 225: one `group id|group name|completion state|last
 * episode number|rating|votes|episode range` line per group.
 */
export const GROUP_STATUS_PARSER = ListField(
  MapFieldStatic({
    gid: Int,
    group_name: StringWithLineBreaks,
    completion_state: CodedField(GROUP_STATUS_CODES),
    last_episode_number: Int,
    rating: Int,
    votes: Int,
    episode_range: String,
  }),
  "\n",
  true,
);

/*
 * UPDATED, reply 243: `entity|total count|last update date|aid list`. The
 * entity is always 1.
 */
export const UPDATED_PARSER = new SimpleParser({
  entity: Int,
  total_count: Int,
  last_update_date: Int,
  updated_aids: ListField(Int, ",", true),
});

/*
 * MYLISTSTATS, reply 222: 17 integer fields in the order of the spec.
 */
export const MYLIST_STATS_PARSER = new SimpleParser({
  anime: Int,
  eps: Int,
  files: Int,
  size_of_files: Int,
  added_animes: Int,
  added_eps: Int,
  added_files: Int,
  added_groups: Int,
  leech_perc: Int,
  glory_perc: Int,
  viewed_perc: Int,
  mylist_perc: Int,
  viewed_mylist_perc: Int,
  viewed_nr_eps: Int,
  votes: Int,
  reviews: Int,
  views_in_min: Int,
});

/*
 * Replies of VOTE (260 to 263, and 360, 363 and 364 as error payloads): `entity
 * name|vote value|vote type|entity id`.
 */
export const VOTE_PARSER = new SimpleParser({
  name: StringWithLineBreaks,
  vote_value: Int,
  type: CodedField(reverseObj(VOTE_TYPES)),
  id: Int,
});

/*
 * RANDOMANIME, reply 230. The spec only says "see ANIME", whose default format
 * has 19 fields, so those are decoded. Extra trailing fields are ignored.
 */
export const RANDOM_PARSER = new SimpleParser({
  aid: String,
  episodes: Int,
  highest_ep_number: Int,
  special_ep_count: Int,
  rating: Int,
  vote_count: Int,
  temp_rating: Int,
  temp_vote_count: Int,
  average_review_rating: Int,
  review_count: Int,
  year: String,
  type: String,
  romanji_name: StringWithLineBreaks,
  kanji_name: StringWithLineBreaks,
  english_name: StringWithLineBreaks,
  other_name_list: ListField(StringWithLineBreaks, undefined, true),
  short_name_list: ListField(StringWithLineBreaks, undefined, true),
  synonym_list: ListField(StringWithLineBreaks, undefined, true),
  categories: ListField(StringWithLineBreaks, ",", true),
});

/*
 * USER, reply 295: `uid|username`.
 */
export const USER_PARSER = new SimpleParser({
  uid: Int,
  username: StringWithLineBreaks,
});

/*
 * The data of a MYLIST by anime reply: 221 is one entry, 312 a summary of
 * several (`anime title|episodes|unknown|hdd|cd|deleted|watched`, then a
 * `group|episode list` pair per group). Any other code, or a group without its
 * episode list, is bad data.
 */
export function parseMylistByAnime(
  code: number,
  data: string,
): MylistByAnimeResult {
  if (code === 221) return { single: MYLIST_PARSER.parseData(data) };
  if (code !== 312) throw badData(`${code} is not a mylist reply`);

  const head = 7;
  const split = data.split("|");
  if (split.length < head) {
    throw badData(`reply has ${split.length} fields, ${head} expected`);
  }
  const groupsPart = split.slice(head);
  if (groupsPart.length % 2 !== 0) {
    throw badData(
      `group ${JSON.stringify(groupsPart.at(-1))} has no episode list`,
    );
  }
  const groups: { group: string; episodes: string }[] = [];
  for (let i = 0; i < groupsPart.length; i += 2) {
    groups.push({
      group: StringWithLineBreaks(groupsPart[i]!),
      episodes: groupsPart[i + 1]!,
    });
  }
  return {
    multi: {
      anime_title: StringWithLineBreaks(split[0]!),
      episodes: parseField("episodes", Int, split[1]!),
      eps_unknown: split[2]!,
      eps_hdd: split[3]!,
      eps_cd: split[4]!,
      eps_deleted: split[5]!,
      eps_watched: split[6]!,
      groups,
    },
  };
}

/*
 * The data of 311 MYLIST ENTRY EDITED: the number of entries, or nothing for
 * one entry.
 */
export function parseMylistEdited(data: string): number {
  return data.trim() === "" ? 1 : Int(data);
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
