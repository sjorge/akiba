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

export const ANIME_DESC_PARSER = new SimpleParser({
  current_part: Number,
  max_parts: Number,
  desc: String,
});

export const CALENDAR_PARSER = ListField(
  MapFieldStatic({
    aid: Number,
    startdate: Number,
    dateflags: Dateflags,
  }),
  "\n",
  true,
);

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
        // TODO: phase 3 Boolean("0") is true, NumBoolean was probably meant
        is_main_seiyuu: Boolean,
      },
      ",",
    ),
    "'",
    true,
  ),
  episode_list: ListField(Number, ",", true),
  last_update_date: String,
  type: Number,
  gender: String,
});

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
        // TODO: phase 3 the spec has 5 => "Now known as" and 6 => "Other"
        relationtype: CodedField({
          "1": "participant_in",
          "2": "parent_of",
          "4": "merged_from",
          "5": "participant_in",
          "6": "participant_in",
          "105": "formerly",
        } as const),
      },
      ",",
    ),
    "'",
  ),
});

export const GROUP_STATUS_CODES = {
  "1": "ongoing",
  "2": "stalled",
  "3": "complete",
  "4": "dropped",
  "5": "finished",
  "6": "specials_only",
} as const;

export const GROUP_STATUS_PARSER = ListField(
  MapFieldStatic({
    gid: Number,
    group_name: String,
    completition_state: CodedField(GROUP_STATUS_CODES),
    last_episode_number: Number,
    rating: Number,
    votes: Number,
    episode_range: String,
  }),
  "\n",
  true,
);

export const UPDATED_PARSER = new SimpleParser({
  entity: Number,
  total_count: Number,
  last_update_date: Number,
  updated_aids: ListField(Number, ",", true),
});

export const MYLIST_STATS_PARSER = new SimpleParser({
  anime: String,
  eps: String,
  files: String,
  size_of_files: String,
  added_animes: String,
  added_eps: String,
  added_files: String,
  added_groups: String,
  leech_perc: String,
  glory_perc: String,
  viewed_perc: String,
  mylist_perc: String,
  viewed_mylist_perc: String,
  viewed_nr_eps: String,
  votes: String,
  reviews: String,
  views_in_min: String,
});

export const VOTE_PARSER = new SimpleParser({
  name: String,
  vote_value: Number,
  type: CodedField(reverseObj(VOTE_TYPES)),
  id: Number,
});

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

export const USER_PARSER = new SimpleParser({ uid: Number, username: String });

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
