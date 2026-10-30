/*
 * Public types of the AniDB UDP client
 */
import type {
  ENCODINGS,
  EXPORT_TEMPLATES,
  MYLIST_FILE_STATES,
  MYLIST_STATES,
  VOTE_TYPES,
} from "lib/anidb/constants";
import type { UdpTransportOptions } from "lib/anidb/udp";

export type CachableValue =
  | string
  | number
  | Buffer
  | null
  | undefined
  | Record<string, unknown>;

export type AnidbCacheImplType = {
  get<T extends CachableValue>(key: string): Promise<T | null | undefined>;
  set: (key: string, value: CachableValue) => Promise<void>;
};

export type AnidbClientOptions = {
  /** accepted for drop-in compatibility, ignored: caching is not part of this client */
  cache?: AnidbCacheImplType;
  /** the local port to listen on for responses (leave undefined if you are not sure) */
  incomming_port?: number;
  /** interval in milliseconds at which anidb is pinged to keep the session alive, defaults to 5 minutes */
  ping_interval?: number;
  /** called once when the client disconnects (replaces the old `disconnect` event) */
  onDisconnect?: (info: { source?: unknown; error?: Error }) => void;
  /** host, port and timing of the UDP transport, only meant for tests */
  transport?: UdpTransportOptions;
};

export type EncodingType = keyof typeof ENCODINGS;
export type MylistState = (typeof MYLIST_STATES)[keyof typeof MYLIST_STATES];
export type MylistFileState = (typeof MYLIST_FILE_STATES)[number];
export type ExportTemplate = (typeof EXPORT_TEMPLATES)[number];
export type VoteType = keyof typeof VOTE_TYPES;

// TODO: phase 3 the old client misspells "end_unkown_month", kept for now
export type DateflagsType =
  | "start_unknown_day"
  | "start_unknown_month"
  | "end_unknown_day"
  | "end_unkown_month"
  | "ended"
  | "start_unknown_year"
  | "end_unknown_year";

export type ParsedResponse = {
  code: number;
  response: Array<string>;
  data: string;
};

export type ConnectResult = {
  session: string;
  ip?: string | null;
  port?: string | null;
};

// TODO: 2.4 FileResult must match the mask definition in parsers
export type FileResult = {
  fid: number;
  aid: number;
  eid: number;
  gid: number;
  mylist_id: number;
  other_episodes: string;
  is_deprecated: boolean;
  state: string;
  size: number;
  ed2k: string;
  md5: string;
  sha1: string;
  crc32: string;
  video_color_depth: string;
  quality: string;
  source: string;
  audio_codecs: string;
  audio_bitrates: string;
  video_codec: string;
  video_bitrate: string;
  video_resolution: string;
  file_extension: string;
  dub_language: string;
  sub_language: string;
  length_seconds: number;
  description: string;
  aired_date: string;
  anidb_filename: string;
  mylist_state: number;
  mylist_filestate: number;
  mylist_viewed: number;
  mylist_viewdate: number;
  mylist_storage: string;
  mylist_source: string;
  mylist_other: string;
  anime_total_episodes: number;
  highest_episode: number;
  year: string;
  type: string;
  related_aid_list: number[];
  related_aid_type: number[];
  category_list: string[];
  romanji_name: string;
  kanji_name: string;
  english_name: string;
  other_name: string;
  short_name_list: string[];
  synonym_list: string[];
  epno: string;
  ep_name: string;
  ep_romanji_name: string;
  ep_kanji_name: string;
  ep_rating: number;
  ep_vote_count: number;
  group_name: string;
  group_short_name: string;
  aid_record_updated: number;
};

// TODO: 2.4 AnimeResult must match the mask definition in parsers
export type AnimeResult = {
  aid: number;
  dateflags: DateflagsType[];
  year: string;
  type: string;
  related_anime: number[];
  related_types: number[];
  romanji_name: string;
  kanji_name: string;
  english_name: string;
  other_name_list: string[];
  short_name_list: string[];
  synonym_list: string[];
  episodes: number;
  highest_ep_number: number;
  special_ep_count: number;
  air_date: number;
  end_date: number;
  url: string;
  picname: string;
  rating: number;
  vote_count: number;
  temp_rating: number;
  temp_vote_count: number;
  average_review_rating: number;
  review_count: number;
  award_list: string[];
  is_18_restricted: boolean;
  ann_id: number;
  allcinema_id: number;
  animenfo_id: string;
  tag_name_list: string[];
  tag_id_list: number[];
  tag_weight_list: number[];
  date_record_updated: string;
  character_id_list: number[];
  specials_count: number;
  credits_count: number;
  other_count: number;
  trailer_count: number;
  parody_count: number;
};

export type AnimeStatus =
  | "ongoing"
  | "stalled"
  | "complete"
  | "dropped"
  | "finished"
  | "specials_only";

export type CalendarEntry = {
  aid: number;
  startdate: number;
  dateflags: Array<string>;
};

export type CharacterResult = {
  readonly charid: number;
  readonly character_name_kanji: string;
  readonly character_name_transc: string;
  readonly pic: string;
  readonly anime_blocks: {
    readonly aid: number;
    readonly appearance: number;
    readonly creatorid: number;
    readonly is_main_seiyuu: boolean;
  }[];
  readonly episode_list: number[];
  readonly last_update_date: string;
  readonly type: number;
  readonly gender: string;
};

export type CreatorResult = {
  readonly creatorid: number;
  readonly creator_name_kanji: string;
  readonly creator_name_transcription: string;
  readonly type: "person" | "company" | "collaboration";
  readonly pic_name: string;
  readonly url_english: string;
  readonly url_japanese: string;
  readonly wiki_url_english: string;
  readonly wiki_url_japanese: string;
  readonly last_update_date: number;
};

export type EpisodeResult = {
  eid: number;
  aid: number;
  length: number;
  rating: number;
  votes: number;
  epno: string;
  eng: string;
  romaji: string;
  kanji: string;
  aired: number;
  type: "" | "S" | "C" | "T" | "P" | "O";
};

export type GroupResult = {
  readonly gid: number;
  readonly rating: number;
  readonly votes: number;
  readonly acount: number;
  readonly fcount: number;
  readonly name: string;
  readonly short: string;
  readonly irc_channel: string;
  readonly irc_server: string;
  readonly url: string;
  readonly picname: string;
  readonly foundeddate: number;
  readonly disbandeddate: number;
  readonly dateflags: DateflagsType[];
  readonly lastreleasedate: number;
  readonly lastactivitydate: number;
  readonly grouprelations: {
    othergroupid: number;
    relationtype: "participant_in" | "parent_of" | "merged_from" | "formerly";
  }[];
};

export type GroupStatusEntry = {
  gid: number;
  group_name: string;
  completition_state: AnimeStatus;
  last_episode_number: number;
  rating: number;
  votes: number;
  episode_range: string;
};

export type UpdatedResult = {
  total_count: number;
  last_update_date: number;
  updated_aids: number[];
};

export type MylistEntry = {
  lid: number;
  fid: number;
  eid: number;
  aid: number;
  gid: number;
  date: number;
  state: MylistState;
  viewdate: number;
  storage: string;
  source: string;
  other: string;
  filestate: MylistFileState;
};

export type MylistByAnimeResult =
  | { single: MylistEntry; multi?: undefined }
  | {
      multi: {
        groups: { group: string; episodes: string }[];
        anime_title: string;
        episodes: number;
        // TODO: phase 3 misspelled field names kept from the old client
        eps_unkown: string;
        eps_hdd: string;
        eps_cd: string;
        eps_deleted: string;
        eps_watiched: string;
      };
      single?: undefined;
    };

export type MylistFileSpec =
  | { fid: number }
  | { ed2k: string; size: number }
  | { lid: number };

export type MylistAddOptions = {
  /** true to edit an existing entry; unsupplied values keep their original value. Implied (and ignored) when `lid` is passed */
  edit?: boolean;
  state?: MylistState;
  viewed?: boolean;
  viewdate?: number;
  /** (i.e. ed2k, dc, ftp, irc, ...) */
  source?: string;
  /** label of the storage */
  storage?: string;
  /** other remarks/notes (`<br/>` can be used to split into multiple lines) */
  other?: string;
};

export type MylistAddByFileResult =
  | { code: "MYLIST_ENTRY_ADDED"; lid: number }
  | { code: "MYLIST_ENTRY_EDITED"; nr_edited: number }
  | undefined;

export type MylistAnimeSpec = {
  anime: string | number;
  group?: string | number;
  epno?: number;
};

export type MylistStatsResult = {
  anime: string;
  eps: string;
  files: string;
  size_of_files: string;
  added_animes: string;
  added_eps: string;
  added_files: string;
  added_groups: string;
  leech_perc: string;
  glory_perc: string;
  viewed_perc: string;
  mylist_perc: string;
  viewed_mylist_perc: string;
  viewed_nr_eps: string;
  votes: string;
  reviews: string;
  views_in_min: string;
};

export type VoteResult = {
  name: string;
  vote_value: number;
  type: VoteType;
  id: number;
  action: "VOTED" | "VOTE_FOUND" | "VOTE_UPDATED" | "VOTE_REVOKED";
  code: 260 | 261 | 262 | 263;
};

export type RandomResult = {
  readonly aid: string;
  readonly episodes: number;
  readonly highest_ep_number: number;
  readonly special_ep_count: number;
  readonly rating: number;
  readonly vote_count: number;
  readonly temp_rating: number;
  readonly temp_vote_count: number;
  readonly average_review_rating: number;
  readonly review_count: number;
  readonly year: string;
  readonly type: string;
  readonly romanji_name: string;
  readonly kanji_name: string;
  readonly english_name: string;
  readonly other_name_list: string[];
  readonly short_name_list: string[];
  readonly synonym_list: string[];
  readonly categories: string[];
  readonly air_date: number;
  readonly end_date: number;
  readonly dateflags: DateflagsType[];
  readonly is_18_restricted: boolean;
  readonly picname: string;
};

export type UserIdResult = {
  uid: number;
  username: string;
};

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
