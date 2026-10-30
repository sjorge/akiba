/*
 * Parsers for the responses of the individual AniDB UDP commands
 */
import {
  CodedField,
  Dateflags,
  ListField,
  MapFieldStatic,
  NumBoolean,
  SimpleParser,
  StringWithLineBreaks,
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
  eid: Number,
  aid: Number,
  length: Number,
  rating: Number,
  votes: Number,
  epno: String,
  eng: String,
  romaji: String,
  kanji: String,
  aired: Number,
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
  lid: Number,
  fid: Number,
  eid: Number,
  aid: Number,
  gid: Number,
  date: Number,
  state: CodedField(MYLIST_STATES),
  viewdate: Number,
  storage: String,
  source: String,
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
export const ANIME_DESC_PARSER = new SimpleParser({
  current_part: Number,
  max_parts: Number,
  desc: String,
});

/*
 * CALENDAR, reply 297: one `aid|start date|date flags` line per title.
 */
export const CALENDAR_PARSER = ListField(
  MapFieldStatic({
    aid: Number,
    startdate: Number,
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
  charid: Number,
  character_name_kanji: String,
  character_name_transc: String,
  pic: String,
  anime_blocks: ListField(
    MapFieldStatic(
      {
        aid: Number,
        appearance: Number,
        creatorid: Number,
        is_main_seiyuu: NumBoolean,
      },
      ",",
    ),
    "'",
    true,
  ),
  episode_list: ListField(Number, ",", true),
  last_update_date: Number,
  type: Number,
  gender: String,
});

/*
 * CREATOR, reply 245: `creatorid|name kanji|name transcription|type|pic|url
 * english|url japanese|wiki url english|wiki url japanese|last update`. Type: 1
 * person, 2 company, 3 collaboration.
 */
export const CREATOR_PARSER = new SimpleParser({
  creatorid: Number,
  creator_name_kanji: String,
  creator_name_transcription: String,
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
  last_update_date: Number,
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
export const GROUP_PARSER = new SimpleParser({
  gid: Number,
  rating: Number,
  votes: Number,
  acount: Number,
  fcount: Number,
  name: String,
  short: String,
  irc_channel: String,
  irc_server: String,
  url: String,
  picname: String,
  foundeddate: Number,
  disbandeddate: Number,
  dateflags: Dateflags,
  lastreleasedate: Number,
  lastactivitydate: Number,
  grouprelations: ListField(
    MapFieldStatic(
      {
        othergroupid: Number,
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
  ),
});

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
    gid: Number,
    group_name: String,
    completion_state: CodedField(GROUP_STATUS_CODES),
    last_episode_number: Number,
    rating: Number,
    votes: Number,
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
  entity: Number,
  total_count: Number,
  last_update_date: Number,
  updated_aids: ListField(Number, ",", true),
});

/*
 * MYLISTSTATS, reply 222: 17 integer fields in the order of the spec.
 */
export const MYLIST_STATS_PARSER = new SimpleParser({
  anime: Number,
  eps: Number,
  files: Number,
  size_of_files: Number,
  added_animes: Number,
  added_eps: Number,
  added_files: Number,
  added_groups: Number,
  leech_perc: Number,
  glory_perc: Number,
  viewed_perc: Number,
  mylist_perc: Number,
  viewed_mylist_perc: Number,
  viewed_nr_eps: Number,
  votes: Number,
  reviews: Number,
  views_in_min: Number,
});

/*
 * Replies of VOTE (260 to 263, and 360, 363 and 364 as error payloads): `entity
 * name|vote value|vote type|entity id`.
 */
export const VOTE_PARSER = new SimpleParser({
  name: String,
  vote_value: Number,
  type: CodedField(reverseObj(VOTE_TYPES)),
  id: Number,
});

/*
 * RANDOMANIME, reply 230. NOTE: the spec only says "see ANIME", whose default
 * format has 19 fields. This definition has 24, as the old client expected
 * them; it was never checked against a live reply.
 */
export const RANDOM_PARSER = new SimpleParser({
  aid: String,
  episodes: Number,
  highest_ep_number: Number,
  special_ep_count: Number,
  rating: Number,
  vote_count: Number,
  temp_rating: Number,
  temp_vote_count: Number,
  average_review_rating: Number,
  review_count: Number,
  year: String,
  type: String,
  romanji_name: String,
  kanji_name: String,
  english_name: String,
  other_name_list: ListField(String, undefined, true),
  short_name_list: ListField(String, undefined, true),
  synonym_list: ListField(String, undefined, true),
  categories: ListField(String, ",", true),
  air_date: Number,
  end_date: Number,
  dateflags: Dateflags,
  is_18_restricted: NumBoolean,
  picname: String,
});

/*
 * USER, reply 295: `uid|username`.
 */
export const USER_PARSER = new SimpleParser({ uid: Number, username: String });

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
