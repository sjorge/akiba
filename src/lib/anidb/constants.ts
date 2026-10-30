/*
 * Constants shared by the AniDB UDP client
 */

/*
 * Server and port of the AniDB UDP API (spec: api.anidb.net, port 9000/UDP).
 * The spec asks front ends to read them from configuration instead of hard
 * coding them, the transport takes a `host` and a `port` option for that.
 */
export const ANIDB_HOST: string = "api.anidb.net";
export const ANIDB_PORT: number = 9000;
/*
 * Version of the API spec this client was written against, sent as `protover`
 * in AUTH. The spec says a client may send nothing but the version of the spec
 * its author used ("3" in the spec this was written from); the server compares
 * it with its own and refuses clients that are too old (503).
 */
export const PROTOCOL_VERSION: number = 3;

// Flood protection (spec): short term at most 0.5 packets per second (enforced
// after the first 5), long term at most one packet per 4 s. Requests run
// strictly one at a time with this spacing, 25 percent above the long term
// limit, so neither rule is ever hit. Do not lower this.
export const REQUEST_INTERVAL_MS: number = 5000;
/*
 * No reply within this time means the packet or the reply was dropped, the
 * server is down or the IP is banned (usually for 30 minutes). Dropped packets
 * still count towards the packet rate, so there is no automatic resend.
 */
export const REQUEST_TIMEOUT_MS: number = 30000;
/*
 * Behind NAT: idle time after which a PING keeps the NAT mapping alive. Routers
 * drop UDP mappings after 5 to 15 minutes of silence and the spec warns that a
 * new mapping (a new port) can get the client banned.
 */
export const PING_INTERVAL_MS: number = 300000;
/*
 * Without NAT: idle time after which UPTIME is sent to keep the session open.
 * The server drops a session after 35 minutes without data and the spec asks
 * for an UPTIME about every 30 minutes.
 */
export const UPTIME_INTERVAL_MS: number = 1800000;

/*
 * Encodings AniDB can switch to, the names of the Java charsets that the spec
 * links to. The server default is ASCII, the client asks for UTF8. An
 * unsupported encoding is silently ignored by AUTH, ENCODING answers 519 for
 * it.
 */
export const ENCODINGS = {
  ISO8859_1: "ISO8859_1",
  ISO8859_2: "ISO8859_2",
  ISO8859_4: "ISO8859_4",
  ISO8859_5: "ISO8859_5",
  ISO8859_7: "ISO8859_7",
  ISO8859_9: "ISO8859_9",
  ISO8859_13: "ISO8859_13",
  ISO8859_15: "ISO8859_15",
  KOI8_R: "KOI8_R",
  ASCII: "ASCII",
  UTF8: "UTF8",
  UTF16: "UTF-16",
  UnicodeBigUnmarked: "UnicodeBigUnmarked",
  UnicodeLittleUnmarked: "UnicodeLittleUnmarked",
  Cp1250: "Cp1250",
  Cp1251: "Cp1251",
  Cp1252: "Cp1252",
  Cp1253: "Cp1253",
  Cp1254: "Cp1254",
  Cp1257: "Cp1257",
  UnicodeBig: "UnicodeBig",
  UnicodeLittle: "UnicodeLittle",
} as const;

/*
 * State of a file in the mylist (spec: MYLIST): 0 unknown, 1 stored on the hdd
 * but not shared, 2 stored on cd/dvd, 3 deleted or otherwise not available
 * (i.e. reencoded), 4 stored on a nas or in the cloud.
 */
export const MYLIST_STATES = {
  "0": "unknown",
  "1": "internal_storage",
  "2": "external_storage",
  "3": "deleted",
  "4": "remote_storage",
} as const;

export const MYLIST_FILE_STATES = [
  "other",
  "normal/original",
  "corrupted version/invalid crc",
  "self edited",
  "self ripped",
  "on dvd",
  "on vhs",
  "on tv",
  "in theaters",
  "streamed",
] as const;

/*
 * Mylist export templates known when this was written. The spec only says that
 * a template name must match one on the MyList Export page of the website, so
 * the server has the last word (317 for unknown ones).
 */
export const EXPORT_TEMPLATES = [
  "csv-adborg",
  "csv-files",
  "csv-minimal",
  "html-anidbstyle",
  "html-cleancut",
  "html-cloud",
  "html-dump",
  "html-fixme",
  "html-fstyle",
  "html-goldsilver",
  "html-graywhite",
  "html-green",
  "html-knch",
  "html-lightblue",
  "html-operatic",
  "html-orangewhite",
  "html-silver",
  "json-adborg",
  "json-large",
  "plot-time-wasted",
  "sfv-simple",
  "txt-ed2k",
  "txt-udp-mylist",
  "xml",
  "xml-acm",
  "xml-anivote",
  "xml-cdb",
  "xml-dark",
  "xml-files",
  "xml-large",
  "xml-mini",
  "xml-mystatus",
  "xml-naive",
  "xml-nx",
  "xml-plain",
  "xml-plain-anisubtwo",
  "xml-plain-cs",
  "xml-plain-full",
  "xml-plain-new",
  "xml-xslt",
] as const;

/*
 * Vote types of the VOTE command (spec: 1 anime, 2 anime temporary vote, 3
 * group, 6 episode).
 */
export const VOTE_TYPES = {
  anime: "1",
  anime_tmp: "2",
  group: "3",
  episode: "6",
} as const;

/*
 * Reply codes above 305 (errors and "nothing found" answers) by name. The names
 * are the strings the server sends, the spec nevertheless asks clients to work
 * with the numbers.
 */
export const ERROR_RESPONSE_CODES = {
  NO_SUCH_FILE: 320,
  NO_SUCH_ENTRY: 321,
  MULTIPLE_FILES_FOUND: 322,
  NO_SUCH_WISHLIST: 323,
  NO_SUCH_NOTIFICATION: 324,
  NO_GROUPS_FOUND: 325,
  NO_SUCH_ANIME: 330,
  NO_SUCH_DESCRIPTION: 333,
  NO_SUCH_REVIEW: 334,
  NO_SUCH_CHARACTER: 335,
  NO_SUCH_SONG: 336,
  NO_SUCH_ANIMETAG: 337,
  NO_SUCH_CHARACTERTAG: 338,
  NO_SUCH_EPISODE: 340,
  NO_SUCH_UPDATES: 343,
  NO_SUCH_TITLES: 344,
  NO_SUCH_CREATOR: 345,
  NO_SUCH_GROUP: 350,
  NO_SUCH_CATEGORY: 351,
  BUDDY_ALREADY_ADDED: 355,
  NO_SUCH_BUDDY: 356,
  BUDDY_ALREADY_ACCEPTED: 357,
  BUDDY_ALREADY_DENIED: 358,
  NO_SUCH_VOTE: 360,
  INVALID_VOTE_TYPE: 361,
  INVALID_VOTE_VALUE: 362,
  PERMVOTE_NOT_ALLOWED: 363,
  ALREADY_PERMVOTED: 364,
  HOT_ANIME_EMPTY: 365,
  RANDOM_RECOMMENDATION_EMPTY: 366,
  RANDOM_SIMILAR_EMPTY: 367,
  NOTIFICATION_DISABLED: 370,
  NO_SUCH_ENTRY_MESSAGE: 381,
  NO_SUCH_ENTRY_NOTIFICATION: 382,
  NO_SUCH_MESSAGE: 392,
  NO_SUCH_NOTIFY: 393,
  NO_SUCH_USER: 394,
  CALENDAR_EMPTY: 397,
  NO_CHANGES: 399,
  NOT_LOGGED_IN: 403,
  NO_SUCH_MYLIST_FILE: 410,
  NO_SUCH_MYLIST_ENTRY: 411,
  MYLIST_UNAVAILABLE: 412,
  LOGIN_FAILED: 500,
  LOGIN_FIRST: 501,
  ACCESS_DENIED: 502,
  CLIENT_VERSION_OUTDATED: 503,
  CLIENT_BANNED: 504,
  ILLEGAL_INPUT_OR_ACCESS_DENIED: 505,
  INVALID_SESSION: 506,
  NO_SUCH_ENCRYPTION_TYPE: 509,
  ENCODING_NOT_SUPPORTED: 519,
  BANNED: 555,
  UNKNOWN_COMMAND: 598,
  INTERNAL_SERVER_ERROR: 600,
  ANIDB_OUT_OF_SERVICE: 601,
  SERVER_BUSY: 602,
  NO_DATA: 603,
  TIMEOUT_DELAY_AND_RESUBMIT: 604,
  API_VIOLATION: 666,
  PUSHACK_CONFIRMED: 701,
  NO_SUCH_PACKET_PENDING: 702,
  VERSION: 998,
  // 670 to 673 are client side codes, never sent by the server and not in the
  // spec
  UNKNOWN_API_ERROR: 670,
  CONNECTION_TIMEOUT: 671,
  NOT_CONNECTED: 672,
  FAILED_TO_INITIALIZE: 673,
} as const;

/*
 * Every reply code of the spec (section Return Codes) by name, plus the client
 * side codes 670 to 673 that the server never sends.
 */
export const RESPONSE_CODE = {
  LOGIN_ACCEPTED: 200,
  LOGIN_ACCEPTED_NEW_VERSION: 201,
  LOGGED_OUT: 203,
  RESOURCE: 205,
  STATS: 206,
  TOP: 207,
  UPTIME: 208,
  ENCRYPTION_ENABLED: 209,
  MYLIST_ENTRY_ADDED: 210,
  MYLIST_ENTRY_DELETED: 211,
  ADDED_FILE: 214,
  ADDED_STREAM: 215,
  EXPORT_QUEUED: 217,
  EXPORT_CANCELLED: 218,
  ENCODING_CHANGED: 219,
  FILE: 220,
  MYLIST: 221,
  MYLIST_STATS: 222,
  WISHLIST: 223,
  NOTIFICATION: 224,
  GROUP_STATUS: 225,
  WISHLIST_ENTRY_ADDED: 226,
  WISHLIST_ENTRY_DELETED: 227,
  WISHLIST_ENTRY_UPDATED: 228,
  MULTIPLE_WISHLIST: 229,
  ANIME: 230,
  ANIME_BEST_MATCH: 231,
  RANDOM_ANIME: 232,
  ANIME_DESCRIPTION: 233,
  REVIEW: 234,
  CHARACTER: 235,
  SONG: 236,
  ANIMETAG: 237,
  CHARACTERTAG: 238,
  EPISODE: 240,
  UPDATED: 243,
  TITLE: 244,
  CREATOR: 245,
  NOTIFICATION_ENTRY_ADDED: 246,
  NOTIFICATION_ENTRY_DELETED: 247,
  NOTIFICATION_ENTRY_UPDATE: 248,
  MULTIPLE_NOTIFICATION: 249,
  GROUP: 250,
  CATEGORY: 251,
  BUDDY_LIST: 253,
  BUDDY_STATE: 254,
  BUDDY_ADDED: 255,
  BUDDY_DELETED: 256,
  BUDDY_ACCEPTED: 257,
  BUDDY_DENIED: 258,
  VOTED: 260,
  VOTE_FOUND: 261,
  VOTE_UPDATED: 262,
  VOTE_REVOKED: 263,
  HOT_ANIME: 265,
  RANDOM_RECOMMENDATION: 266,
  RANDOM_SIMILAR: 267,
  NOTIFICATION_ENABLED: 270,
  NOTIFYACK_SUCCESSFUL_MESSAGE: 281,
  NOTIFYACK_SUCCESSFUL_NOTIFICATION: 282,
  NOTIFICATION_STATE: 290,
  NOTIFYLIST: 291,
  NOTIFYGET_MESSAGE: 292,
  NOTIFYGET_NOTIFY: 293,
  SENDMESSAGE_SUCCESSFUL: 294,
  USER_ID: 295,
  CALENDAR: 297,
  PONG: 300,
  AUTHPONG: 301,
  NO_SUCH_RESOURCE: 305,
  API_PASSWORD_NOT_DEFINED: 309,
  FILE_ALREADY_IN_MYLIST: 310,
  MYLIST_ENTRY_EDITED: 311,
  MULTIPLE_MYLIST_ENTRIES: 312,
  WATCHED: 313,
  SIZE_HASH_EXISTS: 314,
  INVALID_DATA: 315,
  STREAMNOID_USED: 316,
  EXPORT_NO_SUCH_TEMPLATE: 317,
  EXPORT_ALREADY_IN_QUEUE: 318,
  EXPORT_NO_EXPORT_QUEUED_OR_IS_PROCESSING: 319,
  NO_SUCH_FILE: 320,
  NO_SUCH_ENTRY: 321,
  MULTIPLE_FILES_FOUND: 322,
  NO_SUCH_WISHLIST: 323,
  NO_SUCH_NOTIFICATION: 324,
  NO_GROUPS_FOUND: 325,
  NO_SUCH_ANIME: 330,
  NO_SUCH_DESCRIPTION: 333,
  NO_SUCH_REVIEW: 334,
  NO_SUCH_CHARACTER: 335,
  NO_SUCH_SONG: 336,
  NO_SUCH_ANIMETAG: 337,
  NO_SUCH_CHARACTERTAG: 338,
  NO_SUCH_EPISODE: 340,
  NO_SUCH_UPDATES: 343,
  NO_SUCH_TITLES: 344,
  NO_SUCH_CREATOR: 345,
  NO_SUCH_GROUP: 350,
  NO_SUCH_CATEGORY: 351,
  BUDDY_ALREADY_ADDED: 355,
  NO_SUCH_BUDDY: 356,
  BUDDY_ALREADY_ACCEPTED: 357,
  BUDDY_ALREADY_DENIED: 358,
  NO_SUCH_VOTE: 360,
  INVALID_VOTE_TYPE: 361,
  INVALID_VOTE_VALUE: 362,
  PERMVOTE_NOT_ALLOWED: 363,
  ALREADY_PERMVOTED: 364,
  HOT_ANIME_EMPTY: 365,
  RANDOM_RECOMMENDATION_EMPTY: 366,
  RANDOM_SIMILAR_EMPTY: 367,
  NOTIFICATION_DISABLED: 370,
  NO_SUCH_ENTRY_MESSAGE: 381,
  NO_SUCH_ENTRY_NOTIFICATION: 382,
  NO_SUCH_MESSAGE: 392,
  NO_SUCH_NOTIFY: 393,
  NO_SUCH_USER: 394,
  CALENDAR_EMPTY: 397,
  NO_CHANGES: 399,
  NOT_LOGGED_IN: 403,
  NO_SUCH_MYLIST_FILE: 410,
  NO_SUCH_MYLIST_ENTRY: 411,
  MYLIST_UNAVAILABLE: 412,
  LOGIN_FAILED: 500,
  LOGIN_FIRST: 501,
  ACCESS_DENIED: 502,
  CLIENT_VERSION_OUTDATED: 503,
  CLIENT_BANNED: 504,
  ILLEGAL_INPUT_OR_ACCESS_DENIED: 505,
  INVALID_SESSION: 506,
  NO_SUCH_ENCRYPTION_TYPE: 509,
  ENCODING_NOT_SUPPORTED: 519,
  BANNED: 555,
  UNKNOWN_COMMAND: 598,
  INTERNAL_SERVER_ERROR: 600,
  ANIDB_OUT_OF_SERVICE: 601,
  SERVER_BUSY: 602,
  NO_DATA: 603,
  TIMEOUT_DELAY_AND_RESUBMIT: 604,
  API_VIOLATION: 666,
  PUSHACK_CONFIRMED: 701,
  NO_SUCH_PACKET_PENDING: 702,
  VERSION: 998,
  // 670 to 673 are client side codes, never sent by the server and not in the
  // spec
  UNKNOWN_API_ERROR: 670,
  CONNECTION_TIMEOUT: 671,
  NOT_CONNECTED: 672,
  FAILED_TO_INITIALIZE: 673,
} as const;

export type ResponseLabel = keyof typeof RESPONSE_CODE;
export type ResponseCode = (typeof RESPONSE_CODE)[ResponseLabel];

const CODE_TO_LABEL = new Map<number, ResponseLabel>(
  (Object.entries(RESPONSE_CODE) as [ResponseLabel, ResponseCode][]).map(
    ([label, code]) => [code, label],
  ),
);

export function getResponse(code: number | string): ResponseLabel | null {
  return CODE_TO_LABEL.get(Number(code)) ?? null;
}

/*
 * Codes any command may return: 505, 555, 598 and 600, 601, 602, 604, and for
 * commands that need a login 501, 502 and 506 (spec: Basics, general).
 */
export const GLOBAL_ERRORS: ResponseCode[] = [
  RESPONSE_CODE.BANNED,
  RESPONSE_CODE.UNKNOWN_COMMAND,
  RESPONSE_CODE.INTERNAL_SERVER_ERROR,
  RESPONSE_CODE.ILLEGAL_INPUT_OR_ACCESS_DENIED,
  RESPONSE_CODE.ANIDB_OUT_OF_SERVICE,
  RESPONSE_CODE.SERVER_BUSY,
  RESPONSE_CODE.TIMEOUT_DELAY_AND_RESUBMIT,
  RESPONSE_CODE.LOGIN_FIRST,
  RESPONSE_CODE.ACCESS_DENIED,
  RESPONSE_CODE.INVALID_SESSION,
];

/*
 * Codes after which the client closes the connection. NOTE: 501 and 506 are
 * first answered by logging in again (see `callApi`), they only end the
 * connection when that does not help.
 */
export const FATAL_ERROR_CODES: ResponseCode[] = [
  RESPONSE_CODE.ACCESS_DENIED,
  RESPONSE_CODE.CLIENT_VERSION_OUTDATED,
  RESPONSE_CODE.CLIENT_BANNED,
  RESPONSE_CODE.BANNED,
  RESPONSE_CODE.ANIDB_OUT_OF_SERVICE,
  RESPONSE_CODE.LOGIN_FIRST,
  RESPONSE_CODE.INVALID_SESSION,
  RESPONSE_CODE.SERVER_BUSY,
  RESPONSE_CODE.TIMEOUT_DELAY_AND_RESUBMIT,
  RESPONSE_CODE.INTERNAL_SERVER_ERROR,
  RESPONSE_CODE.NO_DATA,
  RESPONSE_CODE.API_VIOLATION,
  RESPONSE_CODE.ENCODING_NOT_SUPPORTED,
  RESPONSE_CODE.UNKNOWN_API_ERROR,
  RESPONSE_CODE.CONNECTION_TIMEOUT,
  RESPONSE_CODE.NOT_CONNECTED,
];

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
