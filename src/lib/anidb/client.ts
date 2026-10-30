/*
 * AniDB UDP API client
 */
import type { ResponseCode } from "lib/anidb/constants";
import type {
  AnidbClientOptions,
  AnimeResult,
  AnimeStatus,
  CalendarEntry,
  CharacterResult,
  ConnectResult,
  CreatorResult,
  EncodingType,
  EpisodeResult,
  ExportTemplate,
  FileResult,
  GroupResult,
  GroupStatusEntry,
  MylistAddByAnimeResult,
  MylistAddByFileResult,
  MylistAddOptions,
  MylistAnimeSpec,
  MylistByAnimeResult,
  MylistEntry,
  MylistFileSpec,
  MylistStatsResult,
  ParsedResponse,
  RandomResult,
  UpdatedResult,
  UserIdResult,
  VoteResult,
  VoteType,
} from "lib/anidb/types";

import {
  ENCODINGS,
  FATAL_ERROR_CODES,
  getResponse,
  MYLIST_STATES,
  PING_INTERVAL_MS,
  UPTIME_INTERVAL_MS,
  PROTOCOL_VERSION,
  RESPONSE_CODE,
  VOTE_TYPES,
} from "lib/anidb/constants";
import { AnidbError } from "lib/anidb/error";
import {
  AnimeParser,
  CodedField,
  FileParser,
  ListField,
  StringWithLineBreaks,
} from "lib/anidb/parsers";
import {
  ANIME_DESC_PARSER,
  CALENDAR_PARSER,
  CHARACTER_PARSER,
  CREATOR_PARSER,
  EPISODE_PARSER,
  GROUP_PARSER,
  GROUP_STATUS_CODES,
  GROUP_STATUS_PARSER,
  MYLIST_PARSER,
  MYLIST_STATS_PARSER,
  RANDOM_PARSER,
  UPDATED_PARSER,
  USER_PARSER,
  VOTE_PARSER,
} from "lib/anidb/responses";
import { UdpTransport, createKey } from "lib/anidb/udp";
import { nameOrId, reverseObj } from "lib/anidb/util";

type Params = Record<string, string | number>;

// the spec: lower case a-z, 4 to 16 characters
const CLIENT_NAME_PATTERN: RegExp = /^[a-z]{4,16}$/;
const DEFAULT_CLIENT_VERSION: number = 1;
const DEFAULT_PRIORITY: number = 5;
const LOGOUT_PRIORITY: number = 0;

// codes after which no request may be sent anymore
const BAN_CODES: ResponseCode[] = [
  RESPONSE_CODE.BANNED,
  RESPONSE_CODE.CLIENT_BANNED,
];

const WAIT_HINT = "Wait at least 30 minutes before trying again.";

/*
 * the server's own text, with a hint added for the codes the spec says to wait
 * on
 */
function errorMessage(code: number, data: string): string | undefined {
  const waitCodes: number[] = [
    ...BAN_CODES,
    RESPONSE_CODE.ANIDB_OUT_OF_SERVICE,
  ];
  if (!waitCodes.includes(code)) return data || undefined;
  return `${data || `${code}: ${getResponse(code)}`}. ${WAIT_HINT}`;
}

// the session is unknown to the server, the spec says to log in again
const SESSION_LOST_CODES: ResponseCode[] = [
  RESPONSE_CODE.LOGIN_FIRST,
  RESPONSE_CODE.INVALID_SESSION,
];

// commands that are part of logging in or out are never retried that way
const NO_RELOGIN_COMMANDS: string[] = ["AUTH", "ENCRYPT", "LOGOUT"];

function toAnidbError(_e: unknown): AnidbError {
  return _e instanceof AnidbError
    ? _e
    : new AnidbError(
        RESPONSE_CODE.UNKNOWN_API_ERROR,
        (_e as Error | undefined)?.message,
      );
}

function illegalInput(message: string): AnidbError {
  return new AnidbError(RESPONSE_CODE.ILLEGAL_INPUT_OR_ACCESS_DENIED, message);
}

/*
 * Client for the AniDB UDP API. Commands are sent one at a time (the rate
 * limiter keeps one request in flight and the flood protection of the spec: no
 * more than one packet every 4 seconds over a longer time, here 5), and a
 * session is kept until `disconnect`. Typical use: `connect` (which logs in),
 * the commands, `disconnect`.
 *
 * NOTE: the spec wants clients to cache what they have asked for, AniDB may ban
 * clients that ask for the same data again and again. This client does not
 * cache, that is up to the caller.
 */
export class AnidbUDPClient {
  private clientId: string;
  private options: AnidbClientOptions;
  private pingInterval: number;
  private uptimeInterval: number;
  // assumed until the AUTH reply proves otherwise, pinging is the safe side
  private behindNat: boolean = true;
  private clientVersion: number;
  private transport: UdpTransport;
  private lastRequest: number = 0;
  private pingTimer: ReturnType<typeof setTimeout> | undefined;
  private disconnecting: Promise<void> | undefined;
  // set after a ban, nothing is sent to anidb again by this client
  private banned: AnidbError | undefined;
  // what `login` needs to log in again after the server lost the session
  private credentials:
    | { user: string; pass: string; encoding?: EncodingType }
    | undefined;

  /*
   * `clientId`: your client's name, register it here:
   * https://anidb.net/software/add. The spec wants 4 to 16 lower case letters
   * and never the name of another client.
   */
  public constructor(clientId: string, options: AnidbClientOptions = {}) {
    this.options = options;
    if (!CLIENT_NAME_PATTERN.test(clientId)) {
      this.warn(
        `AniDB client name "${clientId}" should be 4 to 16 lower case letters (a-z)`,
      );
    }
    this.clientId = clientId;
    this.pingInterval = options.ping_interval || PING_INTERVAL_MS;
    this.uptimeInterval = options.uptime_interval || UPTIME_INTERVAL_MS;
    this.clientVersion = options.client_version ?? DEFAULT_CLIENT_VERSION;
    if (!Number.isInteger(this.clientVersion) || this.clientVersion < 1) {
      throw illegalInput(
        `client_version must be a positive integer but was ${this.clientVersion}`,
      );
    }
    this.transport = new UdpTransport({
      ...options.transport,
      onWarning: (message: string): void => {
        this.warn(message);
      },
    });
  }

  public toString(): string {
    return "AnidbUDPClient";
  }

  // ---- connection commands

  /*
   * Open the local UDP socket and, when user and password are given, log in
   * (AUTH, see `login`). The session is kept alive until `disconnect` is
   * called.
   *
   * With `udp_api_key` an ENCRYPT request is sent first (ENCRYPT user=&type=1,
   * reply 209 with a salt, the AES key is md5(api key + salt)); from then on
   * every packet is encrypted and a normal AUTH is still needed. The api key is
   * the separate UDP API key of the profile page, not the password.
   *
   * NOTE: the spec asks for a fixed local port (too many different ports from
   * one IP get it banned), see the `incomming_port` options.
   */
  public async connect(
    user?: string,
    password?: string,
    udp_api_key?: string,
    encoding?: EncodingType,
  ): Promise<ConnectResult | undefined> {
    if (this.banned) throw this.banned;
    // the old client leaked the previous socket here
    await this.disconnect("connect");

    try {
      await this.openTransport();
    } catch (_e: unknown) {
      throw new AnidbError(
        RESPONSE_CODE.FAILED_TO_INITIALIZE,
        (_e as Error).message,
      );
    }

    try {
      if (udp_api_key && user) {
        const { response } = await this.callApi("ENCRYPT", { user, type: 1 });
        const [salt] = response;
        this.transport.encryptionKey = createKey(udp_api_key, salt!);
      }

      if (user && password) {
        return await this.login(user, password, encoding);
      }
    } catch (_e: unknown) {
      // a connection that could not be set up is not left open
      await this.disconnect("connect", _e as Error);
      throw _e;
    }
    return undefined;
  }

  /*
   * Bind the local socket, a busy fixed port can fall back to a random one.
   */
  private async openTransport(): Promise<void> {
    const port = this.options.incomming_port;
    try {
      await this.transport.open(port);
    } catch (_e: unknown) {
      if ((_e as NodeJS.ErrnoException).code !== "EADDRINUSE" || !port) {
        throw _e;
      }
      const message = `local UDP port ${port} is already in use`;
      if (!this.options.incomming_port_random_fallback) {
        throw new Error(message);
      }
      this.warn(`${message}, using a random port instead`);
      await this.transport.open(null);
    }
  }

  /*
   * PING [nat=1], reply 300 PONG (with nat=1 followed by the port the server
   * sees). No session is needed. Behind NAT a ping keeps the NAT mapping alive,
   * the spec asks not to ping more often than necessary. The client also pings
   * by itself when idle, see `scheduleKeepAlive`.
   */
  public async ping(): Promise<ParsedResponse> {
    return await this.callApi("PING", { nat: 1 }, [RESPONSE_CODE.PONG]);
  }

  /*
   * AUTH user=&pass=&protover=&client=&clientver=[&nat=1&comp=1&enc=], replies
   * 200 or 201 (new client version available) with the session key, 500 LOGIN
   * FAILED, 503 CLIENT VERSION OUTDATED, 504 CLIENT BANNED, 505 and 601.
   *
   * - The password is the normal website password.
   * - The session key (4 to 8 characters of a-z, A-Z, 0-9) is sent as `s` with
   *   every later command and is valid until LOGOUT or until the server has
   *   seen no data for 35 minutes.
   * - `nat=1`: the reply also carries `ip:port` as the server sees it, a
   *   different port than the local one means NAT.
   * - `comp=1`: the server may deflate long replies instead of truncating them.
   * - `enc=`: switches the server encoding right away, unsupported ones are
   *   silently ignored.
   */
  public async login(
    user: string,
    pass: string,
    encoding?: EncodingType,
  ): Promise<ConnectResult> {
    let enc: string = ENCODINGS.UTF8;
    if (encoding) {
      if (!ENCODINGS[encoding]) {
        throw new AnidbError(
          RESPONSE_CODE.ENCODING_NOT_SUPPORTED,
          `${encoding} is not valid, must be one of: ${Object.keys(ENCODINGS).join(", ")}`,
        );
      }
      enc = ENCODINGS[encoding];
    }

    const resp = await this.callApi(
      "AUTH",
      {
        user,
        pass,
        protover: PROTOCOL_VERSION,
        client: this.clientId,
        clientver: this.clientVersion,
        nat: 1,
        comp: 1,
        enc,
      },
      [
        RESPONSE_CODE.LOGIN_ACCEPTED,
        RESPONSE_CODE.LOGIN_ACCEPTED_NEW_VERSION,
        RESPONSE_CODE.LOGIN_FAILED,
      ],
    );

    if (resp.code === RESPONSE_CODE.LOGIN_FAILED) {
      const error = new AnidbError(RESPONSE_CODE.LOGIN_FAILED);
      await this.disconnect("LOGIN_FAILED", error);
      throw error;
    }

    const [session, host] = resp.response;
    const newVersion = resp.code === RESPONSE_CODE.LOGIN_ACCEPTED_NEW_VERSION;
    this.transport.session = session!;
    this.credentials = { user, pass, encoding };

    // with nat=1 the reply carries `ip:port` as the server sees it
    const separator = host?.lastIndexOf(":") ?? -1;
    if (host && separator > 0) {
      const ip = host.slice(0, separator);
      const port = host.slice(separator + 1);
      this.behindNat = Number(port) !== this.transport.localPort;
      this.scheduleKeepAlive();
      return {
        session: session!,
        ip,
        port,
        ...(newVersion && { new_version: true }),
      };
    }
    this.behindNat = true;
    this.scheduleKeepAlive();
    return {
      session: session!,
      ...(newVersion && { new_version: true }),
    };
  }

  /*
   * Log out and close the socket. The spec asks for a LOGOUT whenever the
   * client exits or expects to be idle for 30 minutes or more, and not for a
   * full AUTH, command, LOGOUT cycle per command. Safe to call when not
   * connected and more than once.
   */
  public async disconnect(source?: unknown, error?: Error): Promise<void> {
    if (!this.transport.isOpen) return;
    this.disconnecting ??= this.closeConnection(source, error).finally(() => {
      this.disconnecting = undefined;
    });
    return await this.disconnecting;
  }

  /*
   * LOGOUT, reply 203 LOGGED OUT or 403 NOT LOGGED IN. Only works when logged
   * in. The client stops keeping the session alive afterwards.
   */
  public async logout(priority?: number): Promise<void> {
    const response = await this.callApi(
      "LOGOUT",
      {},
      [RESPONSE_CODE.LOGGED_OUT, RESPONSE_CODE.NOT_LOGGED_IN],
      priority,
    );
    this.transport.session = null;
    if (response.code === RESPONSE_CODE.NOT_LOGGED_IN) {
      throw new AnidbError(RESPONSE_CODE.NOT_LOGGED_IN);
    }
  }

  /*
   * ENCODING name=, reply 219 ENCODING CHANGED or 519 ENCODING NOT SUPPORTED.
   * No session is needed, the encoding is reset to the default on logout.
   * Deprecated: pass the encoding to `login` or `connect` instead (the `enc`
   * option of AUTH is the preferred way).
   */
  public async encoding(
    encoding?: EncodingType,
    priority?: number,
  ): Promise<void> {
    const response = await this.callApi(
      "ENCODING",
      { name: encoding || "UTF8" },
      [RESPONSE_CODE.ENCODING_CHANGED, RESPONSE_CODE.ENCODING_NOT_SUPPORTED],
      priority,
    );
    if (response.code === RESPONSE_CODE.ENCODING_NOT_SUPPORTED) {
      throw new AnidbError(response.code);
    }
  }

  /*
   * VERSION, reply 998 with the server version. No session is needed.
   */
  public async version(): Promise<string> {
    const { data } = await this.callApi("VERSION", {}, [RESPONSE_CODE.VERSION]);
    return data;
  }

  /*
   * UPTIME, reply 208 with the server uptime in milliseconds. The spec calls it
   * the preferred way to check that the session is OK and to keep it alive (the
   * server drops a session after 35 minutes of silence).
   */
  public async uptime(): Promise<number> {
    const { data } = await this.callApi("UPTIME", {}, [RESPONSE_CODE.UPTIME]);
    return Number(data);
  }

  // ---- data commands

  /*
   * ANIME aid= or aname= with an amask, reply 230, or 330 NO SUCH ANIME.
   *
   * - By name the title must match exactly: romaji, kanji, english, other,
   *   synonym or short name.
   * - The fields come back in mask order (byte 1 bit 7 first), see
   *   `AnimeParser`. Asking for a retired, unused or reserved bit gives 505.
   *   `aid` is always requested.
   *
   * NOTE: when the reply would not fit one packet the character id list is cut
   * first, then the tag list, the synonym list and the short name list.
   */
  public async anime<T extends keyof AnimeResult>(
    anime: number | string,
    fields?: Array<T>,
  ): Promise<Pick<AnimeResult, T | "aid">> {
    const amask = new AnimeParser(fields);
    const response = await this.callApi(
      "ANIME",
      nameOrId(anime, "aid", "aname", { amask: amask.mask }),
      [RESPONSE_CODE.ANIME],
    );
    return amask.parseData(response.data) as Pick<AnimeResult, T | "aid">;
  }

  /*
   * ANIMEDESC aid=&part=, reply 233 with `part|max parts|text`, or 330 NO SUCH
   * ANIME, 333 NO SUCH DESCRIPTION. A description can be about 5000 characters
   * but a packet only holds about 1400 bytes, so the text comes in zero based
   * parts that are fetched one by one and joined. By aid only, there is no
   * lookup by title.
   */
  public async animeDesc(aid: number): Promise<string | null | undefined> {
    const command = "ANIMEDESC";
    const { data } = await this.callApi(command, { aid, part: 0 });
    const part1 = ANIME_DESC_PARSER.parseData(data);

    const otherParts: Promise<string>[] = [];
    for (let part = 1; part < part1.max_parts; part++) {
      otherParts.push(
        this.callApi(command, { aid, part }).then(
          ({ data }) => ANIME_DESC_PARSER.parseData(data).desc,
        ),
      );
    }
    // join first: a `<br />` can be cut in two by the part size limit
    return StringWithLineBreaks(
      [part1.desc, ...(await Promise.all(otherParts))].join(""),
    );
  }

  /*
   * CALENDAR, reply 297 with `aid|start date|date flags` lines: the 25 most
   * recently aired titles followed by the next 25, ordered by start date. Takes
   * no parameters. It may be fewer than 50 titles, and adult titles are
   * filtered by the preference of the logged in user. 397 CALENDAR EMPTY gives
   * an empty list.
   */
  public async calendar(): Promise<Array<CalendarEntry>> {
    try {
      const { data } = await this.callApi("CALENDAR", {});
      return CALENDAR_PARSER(data);
    } catch (_e: unknown) {
      // 397: nothing to show, not a failure
      if (
        _e instanceof AnidbError &&
        _e.code === RESPONSE_CODE.CALENDAR_EMPTY
      ) {
        return [];
      }
      throw _e;
    }
  }

  /*
   * CHARACTER charid=, reply 235, or 335 NO SUCH CHARACTER. The anime blocks
   * (`aid,appearance,creatorid,is_main_seiyuu`) are separated by a single
   * quote; without a seiyuu the last two fields are empty but the commas stay.
   * An empty episode list means no episodes were added. Appearance: 0 appears
   * in, 1 cameo, 2 main character, 3 secondary cast. The type and gender codes
   * may change, do not rely on them.
   */
  public async character(charid: number): Promise<CharacterResult> {
    const { data } = await this.callApi("CHARACTER", { charid });
    return CHARACTER_PARSER.parseData(data);
  }

  /*
   * CREATOR creatorid=, reply 245, or 345 NO SUCH CREATOR. Type: 1 person, 2
   * company, 3 collaboration.
   */
  public async creator(creatorid: number): Promise<CreatorResult> {
    const { data } = await this.callApi("CREATOR", { creatorid });
    return CREATOR_PARSER.parseData(data);
  }

  /*
   * EPISODE eid=, reply 240, or 340 NO SUCH EPISODE. The length is in minutes.
   * `epno` carries a prefix for specials (S special, C credits, T trailer, P
   * parody, O other) and is padded for normal episodes; `type` is the raw
   * episode type (1 regular, 2 special, 3 credits, 4 trailer, 5 parody, 6
   * other).
   */
  public async episode(eid: number): Promise<EpisodeResult> {
    const { data } = await this.callApi("EPISODE", { eid });
    return EPISODE_PARSER.parseData(data);
  }

  /*
   * EPISODE by anime (aid= or aname=) and epno=, see `episode`.
   */
  public async episode_by_anime(
    anime: string | number,
    epno: string,
  ): Promise<EpisodeResult> {
    const { data } = await this.callApi(
      "EPISODE",
      nameOrId(anime, "aid", "aname", { epno }),
    );
    return EPISODE_PARSER.parseData(data);
  }

  /*
   * GROUP gid= or gname=, reply 250, or 350 NO SUCH GROUP. `gname` is an exact
   * match of either the name or the short name (both are unique). Needs a
   * login.
   */
  public async group(group: number | string): Promise<GroupResult> {
    const { data } = await this.callApi(
      "GROUP",
      nameOrId(group, "gid", "gname"),
    );
    return GROUP_PARSER.parseData(data);
  }

  /*
   * GROUPSTATUS aid= [state=], reply 225 with one `group id|name|state|last
   * episode|rating|votes|episode range` line per group, or 325 NO SUCH GROUPS
   * FOUND, 330 NO SUCH ANIME.
   *
   * - Without a state the groups that are ongoing, finished or complete are
   *   returned. States: 1 ongoing, 2 stalled, 3 complete, 4 dropped, 5
   *   finished, 6 specials only.
   * - Groups are filtered by the languages of the user's profile and sorted by
   *   descending episode count; groups that do not fit one packet are silently
   *   dropped.
   */
  public async groupstatus(
    aid: number,
    state?: AnimeStatus,
  ): Promise<GroupStatusEntry[]> {
    const request: Params = { aid };
    if (state) {
      const stateCoded = CodedField(reverseObj(GROUP_STATUS_CODES))(state);
      if (stateCoded) request.state = stateCoded;
    }
    const { data } = await this.callApi("GROUPSTATUS", request);
    return GROUP_STATUS_PARSER(data);
  }

  /*
   * FILE fid= with an fmask and an amask, reply 220, or 320 NO SUCH FILE. `fid`
   * always comes first whatever the masks are; the other fields follow the
   * order of the mask tables (fmask before amask), see `FileParser`. There is
   * no way to ask for all fields, and a retired, unused or reserved bit gives
   * 505. Audio codec and bitrate hold several values when the file has several
   * audio streams, separated by a single quote.
   */
  public async file<T extends keyof FileResult>(
    fid: number,
    fields: Array<T>,
  ): Promise<Pick<FileResult, T | "fid">> {
    const fmask = new FileParser(fields);
    const { data } = await this.callApi("FILE", {
      fid,
      fmask: fmask.getFmask(),
      amask: fmask.getAmask(),
    });
    return fmask.parseData(data) as Pick<FileResult, T | "fid">;
  }

  /*
   * FILE by file size and ed2k hash (size=&ed2k=), see `file`. Throws
   * AnidbError 320 (NO_SUCH_FILE) when the hash is unknown, which is how new
   * files are detected.
   */
  public async file_by_hash<T extends keyof FileResult>(
    ed2k: string,
    size: number,
    fields: Array<T>,
  ): Promise<Pick<FileResult, T | "fid">> {
    const fmask = new FileParser(fields);
    const { data } = await this.callApi("FILE", {
      ed2k,
      size,
      fmask: fmask.getFmask(),
      amask: fmask.getAmask(),
    });
    return fmask.parseData(data) as Pick<FileResult, T | "fid">;
  }

  /*
   * FILE by anime (aid= or aname=), group (gid= or gname=) and epno=, see
   * `file`. Only the first matching file is returned by the server. Throws
   * AnidbError MULTIPLE_FILES_FOUND (322, the file ids in `payload`) when more
   * than one file matches.
   */
  public async file_by_episode<T extends keyof FileResult>(
    anime: number | string,
    group: number | string,
    epno: string,
    fields: Array<T>,
  ): Promise<Pick<FileResult, T | "fid">> {
    const fmask = new FileParser(fields);
    let request: Params = {
      epno,
      fmask: fmask.getFmask(),
      amask: fmask.getAmask(),
    };
    request = nameOrId(anime, "aid", "aname", request);
    request = nameOrId(group, "gid", "gname", request);

    const { data, code } = await this.callApi("FILE", request, [
      RESPONSE_CODE.MULTIPLE_FILES_FOUND,
    ]);
    if (code === RESPONSE_CODE.MULTIPLE_FILES_FOUND) {
      const fileLists = ListField(Number, "|", true)(data);
      throw new AnidbError(
        code,
        `Multiple files found ${fileLists.join(", ")}`,
        fileLists as unknown as Record<number, unknown>,
      );
    }
    return fmask.parseData(data) as Pick<FileResult, T | "fid">;
  }

  /*
   * UPDATED entity=1 with either age= (days) or time= (unix time), never both.
   * Reply 243 with `entity|total count|last update date|aid list`, or 343 NO
   * UPDATES, which gives an empty result.
   *
   * - At most 200 ids are returned, a total count above 200 means not all were
   *   retrieved; the last update date is the time of the last aid on the list.
   * - An anime counts as updated when the record changes, a main or official
   *   title is added, edited or deleted, an episode is added or deleted (not
   *   edited), an episode title changes or a relation is added or deleted.
   */
  public async updated(
    opts: { age: number } | { time: number },
  ): Promise<UpdatedResult> {
    const { age, time } = opts as { age?: number; time?: number };
    if (age && time) {
      throw illegalInput("either pass age or time but never both");
    }
    const { data, code } = await this.callApi(
      "UPDATED",
      { entity: 1, ...opts },
      [RESPONSE_CODE.NO_SUCH_UPDATES],
    );
    if (code === RESPONSE_CODE.NO_SUCH_UPDATES) {
      return { total_count: 0, last_update_date: 0, updated_aids: [] };
    }
    const parsed = UPDATED_PARSER.parseData(data);
    return {
      total_count: parsed.total_count,
      last_update_date: parsed.last_update_date,
      updated_aids: parsed.updated_aids,
    };
  }

  // ---- mylist commands

  /*
   * MYLIST lid=, reply 221 with the entry, or 321 NO SUCH ENTRY. State: 0
   * unknown, 1 internal storage (hdd, not shared), 2 external storage (cd/dvd),
   * 3 deleted, 4 remote storage (nas/cloud). File state: 0 normal/original, 1
   * corrupted/invalid crc, 2 self edited, 10 self ripped, 11 dvd, 12 vhs, 13
   * tv, 14 theaters, 15 streamed, 100 other.
   */
  public async mylist_by_id(lid: number): Promise<MylistEntry> {
    const { data } = await this.callApi("MYLIST", { lid });
    return MYLIST_PARSER.parseData(data);
  }

  /*
   * MYLIST fid=, see `mylist_by_id`.
   */
  public async mylist_by_file_id(fid: number): Promise<MylistEntry> {
    const { data } = await this.callApi("MYLIST", { fid });
    return MYLIST_PARSER.parseData(data);
  }

  /*
   * MYLIST size=&ed2k=, see `mylist_by_id`. Throws AnidbError 321
   * (NO_SUCH_ENTRY) when the file is not in the mylist.
   */
  public async mylist_by_file_hash(
    ed2k: string,
    size: number,
  ): Promise<MylistEntry> {
    const { data } = await this.callApi("MYLIST", { ed2k, size });
    return MYLIST_PARSER.parseData(data);
  }

  /*
   * MYLIST by anime (aid= or aname=) with an optional group (gid= or gname=)
   * and epno=. Gives either a single entry (221, `single`) or a summary of
   * several (312 MULTIPLE MYLIST ENTRIES, `multi`): the anime title, the number
   * of episodes, the episodes by state (unknown, hdd, cd, deleted), the watched
   * episodes and then a short name and episode list per group. Episode lists
   * look like `1-12,14-16,T1`.
   */
  public async mylist_by_anime(
    anime: number | string,
    group?: number | string,
    epno?: number,
  ): Promise<MylistByAnimeResult | undefined> {
    let request: Params = {};
    if (epno) request.epno = epno;
    request = nameOrId(anime, "aid", "aname", request);
    if (group) request = nameOrId(group, "gid", "gname", request);

    const { data, code } = await this.callApi("MYLIST", request, [
      RESPONSE_CODE.MYLIST,
      RESPONSE_CODE.MULTIPLE_MYLIST_ENTRIES,
    ]);

    if (code === RESPONSE_CODE.MYLIST) {
      return { single: MYLIST_PARSER.parseData(data) };
    }
    if (code === RESPONSE_CODE.MULTIPLE_MYLIST_ENTRIES) {
      const staticFields = [
        "anime_title",
        "episodes",
        "eps_unknown",
        "eps_hdd",
        "eps_cd",
        "eps_deleted",
        "eps_watched",
      ] as const;
      const split = data.split("|");
      const head = split.slice(0, staticFields.length);
      if (head.length !== staticFields.length) {
        throw new Error(
          `reply has ${head.length} fields, ${staticFields.length} expected`,
        );
      }
      const groupsPart = split.slice(staticFields.length);
      const groups: { group: string; episodes: string }[] = [];
      for (let i = 0; i < groupsPart.length; i += 2) {
        groups.push({ group: groupsPart[i]!, episodes: groupsPart[i + 1]! });
      }
      return {
        multi: {
          anime_title: head[0]!,
          episodes: Number(head[1]),
          eps_unknown: head[2]!,
          eps_hdd: head[3]!,
          eps_cd: head[4]!,
          eps_deleted: head[5]!,
          eps_watched: head[6]!,
          groups,
        },
      };
    }
    return undefined;
  }

  /*
   * MYLISTADD by fid=, by size=&ed2k= or, with edit=1 only, by lid= (`edit` is
   * implied when `lid` is passed). Options:
   * state, viewed, viewdate, source, storage, other (the only field that may
   * hold newlines, sent as `<br />`).
   *
   * - 210 MYLIST ENTRY ADDED with the new lid; 311 MYLIST ENTRY EDITED when
   *   edit=1; 310 FILE ALREADY IN MYLIST with the current record (in `payload`,
   *   the error asks to pass edit=true); 320 NO SUCH FILE; 411 NO SUCH MYLIST
   *   ENTRY.
   * - When editing, values that are not sent keep their value. Without viewdate
   *   the current time is used, and viewdate is ignored for viewed=0.
   * - With the lid the server does less work, use it when the lid is known.
   *   Files added after hashing should get state 1 (on hdd) unless the user
   *   chose otherwise.
   */
  public async mylist_add_by_file(
    fileinfo: MylistFileSpec,
    options?: MylistAddOptions,
  ): Promise<MylistAddByFileResult> {
    const request: Params = {};
    if ("fid" in fileinfo) {
      request.fid = fileinfo.fid;
    } else if ("ed2k" in fileinfo && "size" in fileinfo) {
      request.ed2k = fileinfo.ed2k;
      request.size = fileinfo.size;
    } else if ("lid" in fileinfo) {
      request.lid = fileinfo.lid;
      request.edit = 1;
      if (options && "edit" in options && !options.edit) {
        throw illegalInput(
          "when passing my list id, options.edit must be set to true",
        );
      }
    } else {
      throw illegalInput(
        "fileinfo must be one of { fid: number } { ed2k: string, size: number } or { lid: number }",
      );
    }

    if (options && typeof options === "object") {
      this.addMylistOptions(request, options);
    }
    const { data, code } = await this.callApi("MYLISTADD", request, [
      RESPONSE_CODE.MYLIST_ENTRY_ADDED,
      RESPONSE_CODE.FILE_ALREADY_IN_MYLIST,
      RESPONSE_CODE.MYLIST_ENTRY_EDITED,
    ]);
    switch (code) {
      case RESPONSE_CODE.MYLIST_ENTRY_ADDED:
        return { code: "MYLIST_ENTRY_ADDED", lid: Number(data) };
      case RESPONSE_CODE.MYLIST_ENTRY_EDITED:
        return { code: "MYLIST_ENTRY_EDITED", nr_edited: Number(data) };
      case RESPONSE_CODE.FILE_ALREADY_IN_MYLIST:
        throw new AnidbError(
          RESPONSE_CODE.FILE_ALREADY_IN_MYLIST,
          `File ${JSON.stringify(fileinfo)} already in mylist, pass edit=true to edit`,
          MYLIST_PARSER.parseData(data),
        );
      default:
        throw new AnidbError(
          RESPONSE_CODE.UNKNOWN_API_ERROR,
          `Unkown response code returned ${code}`,
        );
    }
  }

  /*
   * MYLISTADD by anime (aid= or aname=), group (gid=, gname= or generic=1) and
   * epno=, with the options of `mylist_add_by_file`. Reply 210 with the number
   * of entries added, or 311 with the number edited when edit=1; 330 NO SUCH
   * ANIME, 350 NO SUCH GROUP, 322 MULTIPLE FILES FOUND.
   *
   * - epno 0 means all episodes (the default), a negative number means up to
   *   that episode (-12 is up to 12).
   * - Without a group `generic=1` is sent, so the generic file is added instead
   *   of the episode for every group. The spec makes the group optional only
   *   for edit=1.
   */
  public async mylist_add_by_anime(
    animeinfo: MylistAnimeSpec,
    options?: MylistAddOptions,
  ): Promise<MylistAddByAnimeResult> {
    if (
      !animeinfo ||
      !animeinfo.anime ||
      (animeinfo.epno && !Number.isInteger(animeinfo.epno))
    ) {
      throw illegalInput(
        "Invalid first argument: animeinfo.anime property and animeinfo.epno properties are required",
      );
    }

    let request: Params = nameOrId(animeinfo.anime, "aid", "aname");
    if (animeinfo.group !== undefined) {
      request = nameOrId(animeinfo.group, "gid", "gname", request);
    } else {
      request.generic = 1;
    }
    request.epno = animeinfo.epno ? Number(animeinfo.epno) : 0;

    if (options && typeof options === "object") {
      this.addMylistOptions(request, options);
    }
    const { data, code } = await this.callApi("MYLISTADD", request, [
      RESPONSE_CODE.MYLIST_ENTRY_ADDED,
      RESPONSE_CODE.MYLIST_ENTRY_EDITED,
    ]);
    return code === RESPONSE_CODE.MYLIST_ENTRY_EDITED
      ? { edited: Number(data) }
      : { added: Number(data) };
  }

  /*
   * MYLISTDEL by lid=, fid= or size=&ed2k=, reply 211 MYLIST ENTRY DELETED with
   * the number of entries, or 411 NO SUCH MYLIST ENTRY. Deletes all entries
   * found.
   */
  public async mylist_del_by_file(
    fileinfo: MylistFileSpec,
  ): Promise<{ removed: number }> {
    const request: Params = {};
    if ("fid" in fileinfo) {
      request.fid = fileinfo.fid;
    } else if ("ed2k" in fileinfo && "size" in fileinfo) {
      request.ed2k = fileinfo.ed2k;
      request.size = fileinfo.size;
    } else if ("lid" in fileinfo) {
      request.lid = fileinfo.lid;
    } else {
      throw illegalInput(
        "fileinfo must be one of { fid: number } { ed2k: string, size: number } or { lid: number }",
      );
    }
    const { data } = await this.callApi("MYLISTDEL", request, [
      RESPONSE_CODE.MYLIST_ENTRY_DELETED,
    ]);
    return { removed: Number(data) };
  }

  /*
   * MYLISTDEL by anime (aid= or aname=) with an optional group (gid= or gname=)
   * and epno=, see `mylist_del_by_file`. Without an epno every matching entry
   * is deleted.
   */
  public async mylist_del_by_anime(animeinfo: {
    anime: string | number;
    group?: string | number;
    epno?: number;
  }): Promise<{ removed: number }> {
    if (
      !animeinfo ||
      !animeinfo.anime ||
      (animeinfo.epno !== undefined && !Number.isInteger(animeinfo.epno))
    ) {
      throw illegalInput(
        "Invalid first argument: animeinfo.anime is required and animeinfo.epno must be an integer when given",
      );
    }

    let request: Params = nameOrId(animeinfo.anime, "aid", "aname");
    if (animeinfo.group !== undefined) {
      request = nameOrId(animeinfo.group, "gid", "gname", request);
    }
    // without an epno every matching entry is deleted
    if (animeinfo.epno !== undefined) request.epno = animeinfo.epno;
    const { data } = await this.callApi("MYLISTDEL", request, [
      RESPONSE_CODE.MYLIST_ENTRY_DELETED,
    ]);
    return { removed: Number(data) };
  }

  /*
   * MYLISTSTATS, reply 222 with 17 integer fields: animes, episodes, files,
   * size of files, added animes, added episodes, added files, added groups,
   * leech %, glory %, viewed % of db, mylist % of db, viewed % of mylist,
   * number of viewed episodes, votes, reviews and viewed length in minutes.
   */
  public async mylist_stats(): Promise<MylistStatsResult> {
    const { data } = await this.callApi("MYLISTSTATS", {});
    return MYLIST_STATS_PARSER.parseData(data);
  }

  // ---- misc commands

  /*
   * VOTE type=&id= (or name=) [&value=][&epno=]. Types: 1 anime, 2 anime
   * temporary vote, 3 group, 6 episode. Value: a negative number revokes, 0
   * retrieves (the default), 100 to 1000 are valid votes, anything else is
   * illegal. Votes are updated without asking, and a temporary vote is not
   * possible when a permanent one exists.
   *
   * - Replies carry `entity name|value|type|entity id`: 260 VOTED, 261 VOTE
   *   FOUND, 262 VOTE UPDATED (old value), 263 VOTE REVOKED, 360 NO SUCH VOTE,
   *   363 PERMVOTE NOT ALLOWED, 364 ALREADY PERMVOTED; 361 and 362 are invalid
   *   type and value.
   * - For an episode vote add `epno` with type 1, or use type 6 when the
   *   episode id is known.
   *
   * Here `vote` is 'get' (value 0), 'revoke' (value -1) or a number from 100 to
   * 1000, and `epno` is only allowed with type `anime`.
   */
  public async vote(
    type: VoteType,
    name_or_id: string | number,
    vote: "get" | "revoke" | number,
    epno?: string | number,
  ): Promise<VoteResult> {
    if (!(type in VOTE_TYPES)) {
      throw illegalInput(
        `type: ${type} must be one of ${Object.keys(VOTE_TYPES).join(", ")}`,
      );
    }
    if (epno !== undefined && type !== "anime") {
      throw illegalInput("epno can only be used when voting for an anime");
    }

    let request: Params = { type: VOTE_TYPES[type] };
    request = nameOrId(name_or_id, "id", "name", request);
    if (epno !== undefined) request.epno = epno;
    if (vote === "get" || !vote) {
      request.value = 0;
    } else if (vote === "revoke") {
      request.value = -1;
    } else if (Number.isInteger(vote) && vote >= 100 && vote <= 1000) {
      request.value = vote;
    } else {
      throw illegalInput(
        `vote (${vote}) must must be 'get' or 'revoke' or a number between 100 and 1000 `,
      );
    }

    const actions: Record<number, VoteResult["action"]> = {
      [RESPONSE_CODE.VOTED]: "VOTED",
      [RESPONSE_CODE.VOTE_FOUND]: "VOTE_FOUND",
      [RESPONSE_CODE.VOTE_REVOKED]: "VOTE_REVOKED",
      [RESPONSE_CODE.VOTE_UPDATED]: "VOTE_UPDATED",
    };

    try {
      const response = await this.callApi(
        "VOTE",
        request,
        Object.keys(actions).map(Number) as ResponseCode[],
      );
      return {
        action: actions[response.code]!,
        code: response.code as VoteResult["code"],
        ...VOTE_PARSER.parseData(response.data),
      };
    } catch (_e: unknown) {
      if (
        _e instanceof AnidbError &&
        (_e.code === RESPONSE_CODE.NO_SUCH_VOTE ||
          _e.code === RESPONSE_CODE.PERMVOTE_NOT_ALLOWED ||
          _e.code === RESPONSE_CODE.ALREADY_PERMVOTED)
      ) {
        _e.payload = { code: _e.code, ...VOTE_PARSER.parseData(_e.message) };
      }
      throw _e;
    }
  }

  /*
   * RANDOMANIME type=, reply 230 in the format of ANIME without an amask. Type:
   * 0 from the database, 1 watched, 2 unwatched, 3 all of the mylist.
   */
  public async random(
    type: "db" | "watched" | "unwatched" | "mylist",
  ): Promise<RandomResult> {
    const types = { db: 0, watched: 1, unwatched: 2, mylist: 3 } as const;
    if (!(type in types)) {
      throw illegalInput(
        `${type} not a valid type, must be one of: ${Object.keys(types).join(", ")}`,
      );
    }
    const { data } = await this.callApi("RANDOMANIME", { type: types[type] });
    return RANDOM_PARSER.parseData(data);
  }

  /*
   * MYLISTEXPORT template=, reply 217 EXPORT QUEUED, 317 EXPORT NO SUCH
   * TEMPLATE or 318 EXPORT ALREADY IN QUEUE. Only one export can be queued at a
   * time. The servers export when load is low, which may take up to 24 hours,
   * and send an AniDB message when it is ready. The template must be one of the
   * templates on the MyList Export page of the website.
   */
  public async request_mylist_export(
    template: ExportTemplate,
  ): Promise<"EXPORT_QUEUED"> {
    await this.callApi("MYLISTEXPORT", { template }, [
      RESPONSE_CODE.EXPORT_QUEUED,
    ]);
    return "EXPORT_QUEUED";
  }

  /*
   * MYLISTEXPORT cancel=1, reply 218 EXPORT CANCELLED, or 319 EXPORT NO EXPORT
   * QUEUED OR IS PROCESSING. Cancels any pending export, also one queued
   * through the website.
   */
  public async cancel_mylist_export(): Promise<"EXPORT_CANCELLED"> {
    await this.callApi("MYLISTEXPORT", { cancel: 1 }, [
      RESPONSE_CODE.EXPORT_CANCELLED,
    ]);
    return "EXPORT_CANCELLED";
  }

  /*
   * SENDMSG to=&title=&body=, reply 294 SENDMSG SUCCESSFUL, 394 NO SUCH USER or
   * 501 LOGIN FIRST. The title may be 50 characters at most, the body 900.
   */
  public async send_message(
    username: string,
    title: string,
    body: string,
  ): Promise<string> {
    if (
      !username ||
      typeof username !== "string" ||
      !title ||
      typeof title !== "string" ||
      !body ||
      typeof body !== "string"
    ) {
      throw illegalInput(
        "Fields username, title, and body are required and must be a string",
      );
    }
    if (title.length > 50) {
      throw illegalInput(
        `title must be shorter than 50 characters but was ${title.length}`,
      );
    }
    if (body.length > 900) {
      throw illegalInput(
        `body must be shorter than 900 characters but was ${body.length}`,
      );
    }
    await this.callApi("SENDMSG", { to: username, title, body });
    return "SENDMESSAGE_SUCCESSFUL";
  }

  /*
   * USER user= or uid=, reply 295 with `uid|username`, or 394 NO SUCH USER.
   */
  public get_user_id(username: string): Promise<UserIdResult>;
  public get_user_id(uid: number): Promise<UserIdResult>;
  public async get_user_id(user: string | number): Promise<UserIdResult> {
    const { data } = await this.callApi(
      "USER",
      nameOrId(user, "uid", "user", {}),
    );
    return USER_PARSER.parseData(data);
  }

  // ---- internals

  /*
   * the library does not print anything itself, the host decides
   */
  private warn(message: string): void {
    this.options.onWarning?.(message);
  }

  /*
   * Send a command. Codes in `expected` are returned as they are, other codes
   * above 305 become an AnidbError. Fatal errors disconnect the client.
   */
  private async callApi(
    command: string,
    params: Params,
    expected: ResponseCode[] = [],
    priority: number = DEFAULT_PRIORITY,
  ): Promise<ParsedResponse> {
    let error: AnidbError;
    try {
      return await this.sendRequest(command, params, expected, priority);
    } catch (_e: unknown) {
      error = toAnidbError(_e);
    } finally {
      this.lastRequest = Date.now();
    }

    // the server lost our session: log in again and send the command once more
    if (
      SESSION_LOST_CODES.includes(error.code) &&
      !NO_RELOGIN_COMMANDS.includes(command) &&
      this.credentials
    ) {
      try {
        const { user, pass, encoding } = this.credentials;
        this.transport.session = null;
        await this.login(user, pass, encoding);
        return await this.sendRequest(command, params, expected, priority);
      } catch (_e: unknown) {
        error = toAnidbError(_e);
      } finally {
        this.lastRequest = Date.now();
      }
    }

    if (BAN_CODES.includes(error.code)) this.banned = error;
    if (FATAL_ERROR_CODES.includes(error.code)) {
      try {
        await this.disconnect(`callApi.${command}`, error);
      } catch (_dc: unknown) {
        this.warn(
          `Could not properly disconnect after fatal error: ${(_dc as Error).message}`,
        );
      }
    }
    throw error;
  }

  /*
   * One request, codes above 305 that are not expected become an AnidbError.
   */
  private async sendRequest(
    command: string,
    params: Params,
    expected: ResponseCode[],
    priority: number,
  ): Promise<ParsedResponse> {
    if (this.banned) throw this.banned;
    if (!this.transport.isOpen) {
      throw new AnidbError(RESPONSE_CODE.NOT_CONNECTED);
    }
    const response = await this.transport.request(command, params, priority);
    if ((expected as number[]).includes(response.code)) return response;
    if (response.code > 305) {
      throw new AnidbError(
        response.code as ResponseCode,
        errorMessage(response.code, response.data),
      );
    }
    return response;
  }

  private async closeConnection(
    source?: unknown,
    error?: Error,
  ): Promise<void> {
    try {
      // no more packets for a server that just banned us
      if (this.transport.session && !this.banned) {
        await this.transport.request("LOGOUT", {}, LOGOUT_PRIORITY);
      }
    } finally {
      clearTimeout(this.pingTimer);
      this.pingTimer = undefined;
      this.credentials = undefined;
      await this.transport.close();
      this.options.onDisconnect?.({ source, error });
    }
  }

  /*
   * Keep the connection alive while it is idle: PING behind NAT (keeps the NAT
   * mapping), UPTIME otherwise (keeps the session, the server drops it after
   * 35 minutes without data).
   */
  private scheduleKeepAlive(): void {
    clearTimeout(this.pingTimer);
    if (!this.transport.session) return;
    const wait = Math.max(this.lastRequest + this.idleLimit() - Date.now(), 0);
    this.pingTimer = setTimeout(() => {
      void this.keepAlive();
    }, wait);
    this.pingTimer.unref();
  }

  private idleLimit(): number {
    return this.behindNat ? this.pingInterval : this.uptimeInterval;
  }

  private async keepAlive(): Promise<void> {
    if (!this.transport.session) return;
    try {
      if (Date.now() > this.lastRequest + this.idleLimit()) {
        if (this.behindNat) await this.ping();
        else await this.uptime();
      }
    } catch (_e: unknown) {
      try {
        await this.disconnect("autoPing", _e as Error);
      } catch (_dc: unknown) {
        this.warn(
          `Could not properly disconnect after failed ping: ${(_dc as Error).message}`,
        );
      }
      return;
    }
    this.scheduleKeepAlive();
  }

  /*
   * the request parameters of the mylist options, undefined ones are skipped
   */
  private addMylistOptions(request: Params, options: MylistAddOptions): void {
    for (const [label, value] of Object.entries(options)) {
      if (value === undefined) continue;
      switch (label) {
        case "viewed":
        case "edit":
          request[label] = value ? 1 : 0;
          break;
        case "state":
          request[label] = CodedField(reverseObj(MYLIST_STATES))(
            value as string,
          );
          break;
        default:
          request[label] = value as string | number;
      }
    }
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
