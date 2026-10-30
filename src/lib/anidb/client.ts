/*
 * AniDB UDP API client
 */
import {
  ENCODINGS,
  EXPORT_TEMPLATES,
  FATAL_ERROR_CODES,
  MYLIST_STATES,
  PING_INTERVAL_MS,
  PROTOCOL_VERSION,
  RESPONSE_CODE,
  VOTE_TYPES,
} from "lib/anidb/constants";
import type { ResponseCode } from "lib/anidb/constants";
import { AnidbError } from "lib/anidb/error";
import { log } from "lib/logger";
import {
  AnimeParser,
  CodedField,
  FileParser,
  ListField,
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
import { UdpTransport, createKey } from "lib/anidb/udp";
import { nameOrId, reverseObj } from "lib/anidb/util";

type Params = Record<string, string | number>;

const DEFAULT_CLIENT_ID: string = "anidbudpclientjs";
const DEFAULT_PRIORITY: number = 5;
const LOGOUT_PRIORITY: number = 0;

function illegalInput(message: string): AnidbError {
  return new AnidbError(RESPONSE_CODE.ILLEGAL_INPUT_OR_ACCESS_DENIED, message);
}

export class AnidbUDPClient {
  private clientId: string;
  private options: AnidbClientOptions;
  private pingInterval: number;
  private transport: UdpTransport;
  private lastRequest: number = 0;
  private pingTimer: ReturnType<typeof setTimeout> | undefined;
  private disconnecting: Promise<void> | undefined;

  /**
   * @param clientId client id, add your own here: https://anidb.net/software/add
   */
  constructor(clientId?: string, options: AnidbClientOptions = {}) {
    this.clientId = clientId || DEFAULT_CLIENT_ID;
    this.options = options;
    this.pingInterval = options.ping_interval || PING_INTERVAL_MS;
    this.transport = new UdpTransport(options.transport);
  }

  /**
   * Compat shim for the EventEmitter based old client.
   * TODO: phase 3 remove, together with the call in renamer.ts
   */
  setMaxListeners(n: number): this {
    void n;
    return this;
  }

  // ---- connection commands

  /**
   * Start the connection to anidb, logging in when user and password are
   * given. The session is kept alive until `disconnect` is called.
   */
  async connect(
    user?: string,
    password?: string,
    udp_api_key?: string,
    encoding?: EncodingType,
  ): Promise<ConnectResult | undefined> {
    // the old client leaked the previous socket here
    await this.disconnect("connect");

    try {
      await this.transport.open(this.options.incomming_port);
    } catch (_e) {
      throw new AnidbError(
        RESPONSE_CODE.FAILED_TO_INITIALIZE,
        (_e as Error).message,
      );
    }

    if (udp_api_key && user) {
      const { response } = await this.callApi("ENCRYPT", { user, type: 1 });
      const [salt] = response;
      this.transport.encryptionKey = createKey(udp_api_key, salt!);
    }

    if (user && password) {
      return await this.login(user, password, encoding);
    }
    return undefined;
  }

  /** Manually ping, the client also pings by itself when idle. */
  async ping(): Promise<ParsedResponse> {
    return await this.callApi("PING", { nat: 1 }, [RESPONSE_CODE.PONG]);
  }

  async login(
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
        clientver: 1,
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
    this.transport.session = session!;
    this.scheduleKeepAlive();

    if (resp.response.length > 4) {
      const [ip, port] = host!.split(":");
      return { session: session!, ip, port };
    }
    return { session: session! };
  }

  /**
   * Disconnect and logout. Safe to call when not connected, and more than
   * once.
   */
  async disconnect(source?: unknown, error?: Error): Promise<void> {
    if (!this.transport.isOpen) return;
    this.disconnecting ??= this.closeConnection(source, error).finally(() => {
      this.disconnecting = undefined;
    });
    return await this.disconnecting;
  }

  /**
   * Logs the user out. The client stops keeping the session alive when logged
   * out.
   */
  async logout(priority?: number): Promise<void> {
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

  /**
   * Change the encoding for future responses (reset on disconnect).
   * @deprecated pass the encoding to `login` or `connect` instead
   */
  async encoding(encoding?: EncodingType, priority?: number): Promise<void> {
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

  /** Retrieve server uptime */
  async uptime(): Promise<number> {
    const { data } = await this.callApi("UPTIME", {}, [RESPONSE_CODE.UPTIME]);
    return Number(data);
  }

  // ---- data commands

  /** Anime by title (any synonym) or aid */
  async anime<T extends keyof AnimeResult>(
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

  async animeDesc(aid: number): Promise<string | null | undefined> {
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
    return [part1.desc, ...(await Promise.all(otherParts))].join("");
  }

  async calendar(): Promise<Array<CalendarEntry>> {
    const { data } = await this.callApi("CALENDAR", {});
    return CALENDAR_PARSER(data);
  }

  async character(charid: number): Promise<CharacterResult> {
    const { data } = await this.callApi("CHARACTER", { charid });
    return CHARACTER_PARSER.parseData(data);
  }

  async creator(creatorid: number): Promise<CreatorResult> {
    const { data } = await this.callApi("CREATOR", { creatorid });
    return CREATOR_PARSER.parseData(data);
  }

  async episode(eid: number): Promise<EpisodeResult> {
    const { data } = await this.callApi("EPISODE", { eid });
    return EPISODE_PARSER.parseData(data);
  }

  /** Episode by anime (title or aid) and episode number */
  async episode_by_anime(
    anime: string | number,
    epno: string,
  ): Promise<EpisodeResult> {
    const { data } = await this.callApi(
      "EPISODE",
      nameOrId(anime, "aid", "aname", { epno }),
    );
    return EPISODE_PARSER.parseData(data);
  }

  /** Group by id or name */
  async group(group: number | string): Promise<GroupResult> {
    const { data } = await this.callApi(
      "GROUP",
      nameOrId(group, "gid", "gname"),
    );
    return GROUP_PARSER.parseData(data);
  }

  /** Groups and episode ranges released by them for an anime */
  async groupstatus(
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

  async file<T extends keyof FileResult>(
    fid: number,
    fields: Array<T>,
  ): Promise<Pick<FileResult, T | "fid">> {
    const fmask = new FileParser(fields);
    const { data } = await this.callApi("FILE", {
      fid,
      fmask: fmask.getFmask(),
      amask: fmask.getAmask(),
    });
    // TODO: phase 3 fid is a string at runtime, see FileParseResult
    return fmask.parseData(data) as unknown as Pick<FileResult, T | "fid">;
  }

  /** Throws AnidbError 320 (NO_SUCH_FILE) when the hash is unknown. */
  async file_by_hash<T extends keyof FileResult>(
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
    // TODO: phase 3 fid is a string at runtime, see FileParseResult
    return fmask.parseData(data) as unknown as Pick<FileResult, T | "fid">;
  }

  /**
   * Throws AnidbError MULTIPLE_FILES_FOUND (file ids in `payload`) when more
   * than one file matches.
   */
  async file_by_episode<T extends keyof FileResult>(
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
    // TODO: phase 3 fid is a string at runtime, see FileParseResult
    return fmask.parseData(data) as unknown as Pick<FileResult, T | "fid">;
  }

  /** Anime updated in a time frame: either `age` (days) or `time` (unix) */
  async updated(
    opts: { age: number } | { time: number },
  ): Promise<UpdatedResult> {
    const { age, time } = opts as { age?: number; time?: number };
    if (age && time) {
      throw illegalInput("either pass age or time but never both");
    }
    const { data } = await this.callApi("UPDATED", { entity: 1, ...opts });
    const parsed = UPDATED_PARSER.parseData(data);
    return {
      total_count: parsed.total_count,
      last_update_date: parsed.last_update_date,
      updated_aids: parsed.updated_aids,
    };
  }

  // ---- mylist commands

  async mylist_by_id(lid: number): Promise<MylistEntry> {
    const { data } = await this.callApi("MYLIST", { lid });
    return MYLIST_PARSER.parseData(data);
  }

  async mylist_by_file_id(fid: number): Promise<MylistEntry> {
    const { data } = await this.callApi("MYLIST", { fid });
    return MYLIST_PARSER.parseData(data);
  }

  /** Throws AnidbError 321 (NO_SUCH_ENTRY) when the file is not in the mylist. */
  async mylist_by_file_hash(ed2k: string, size: number): Promise<MylistEntry> {
    const { data } = await this.callApi("MYLIST", { ed2k, size });
    return MYLIST_PARSER.parseData(data);
  }

  /** Either a single entry (`single`) or a summary of several (`multi`) */
  async mylist_by_anime(
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
      // TODO: phase 3 misspelled field names kept from the old client
      const staticFields = [
        "anime_title",
        "episodes",
        "eps_unkown",
        "eps_hdd",
        "eps_cd",
        "eps_deleted",
        "eps_watiched",
      ] as const;
      const split = data.split("|");
      const head = split.slice(0, staticFields.length);
      if (head.length !== staticFields.length) {
        throw new Error(
          `Definition and input item length missmatch input: ${head.length} definition: ${staticFields.length}`,
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
          eps_unkown: head[2]!,
          eps_hdd: head[3]!,
          eps_cd: head[4]!,
          eps_deleted: head[5]!,
          eps_watiched: head[6]!,
          groups,
        },
      };
    }
    return undefined;
  }

  /**
   * Add a mylist entry by file, or edit it with `options.edit` (implied when
   * passing `lid`). Throws AnidbError FILE_ALREADY_IN_MYLIST (entry in
   * `payload`) unless `edit` is set.
   */
  async mylist_add_by_file(
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

    // TODO: phase 3 without an options object nothing is sent and undefined
    // is returned, as in the old client
    if (!options || typeof options !== "object") return undefined;

    this.addMylistOptions(request, options);
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

  /**
   * Add mylist entries by anime. `epno` 0 means all episodes (default),
   * negative numbers mean up to.
   */
  async mylist_add_by_anime(
    animeinfo: MylistAnimeSpec,
    options?: MylistAddOptions,
  ): Promise<{ added: number }> {
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
    if ("group" in animeinfo) {
      request = nameOrId(animeinfo.group!, "gid", "gname", request);
    } else {
      request.generic = 1;
    }
    request.epno = animeinfo.epno ? Number(animeinfo.epno) : 0;

    if (options && typeof options === "object") {
      this.addMylistOptions(request, options);
    }
    const { data } = await this.callApi("MYLISTADD", request, [
      RESPONSE_CODE.MYLIST_ENTRY_ADDED,
    ]);
    return { added: Number(data) };
  }

  async mylist_del_by_file(
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
    // TODO: phase 3 the old client expects MYLIST_ENTRY_ADDED here, the reply
    // is MYLIST_ENTRY_DELETED (211), which is below 305 and so passes anyway
    const { data } = await this.callApi("MYLISTDEL", request, [
      RESPONSE_CODE.MYLIST_ENTRY_ADDED,
    ]);
    return { removed: Number(data) };
  }

  async mylist_del_by_anime(animeinfo: {
    anime: string | number;
    group?: string | number;
    epno: number;
  }): Promise<{ removed: number }> {
    if (
      !animeinfo ||
      !animeinfo.anime ||
      (!animeinfo.epno && animeinfo.epno !== 0) ||
      !Number.isInteger(animeinfo.epno)
    ) {
      throw illegalInput(
        "Invalid first argument: animeinfo.anime property and animeinfo.epno properties are required",
      );
    }

    let request: Params = nameOrId(animeinfo.anime, "aid", "aname");
    if ("group" in animeinfo) {
      request = nameOrId(animeinfo.group!, "gid", "gname", request);
    }
    request.epno = Number(animeinfo.epno);
    const { data } = await this.callApi("MYLISTDEL", request, [
      RESPONSE_CODE.MYLIST_ENTRY_ADDED,
    ]);
    return { removed: Number(data) };
  }

  async mylist_stats(): Promise<MylistStatsResult> {
    const { data } = await this.callApi("MYLISTSTATS", {});
    return MYLIST_STATS_PARSER.parseData(data);
  }

  // ---- misc commands

  /**
   * Vote for an anime, episode or group. `vote` is 'get', 'revoke' or a number
   * between 100 and 1000.
   */
  async vote(
    type: VoteType,
    name_or_id: string | number,
    vote: "get" | "revoke" | number,
  ): Promise<VoteResult> {
    if (!(type in VOTE_TYPES)) {
      throw illegalInput(
        `type: ${type} must be one of ${Object.keys(VOTE_TYPES).join(", ")}`,
      );
    }

    let request: Params = { type: VOTE_TYPES[type] };
    request = nameOrId(name_or_id, "id", "name", request);
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
    } catch (_e) {
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

  async random(
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

  /**
   * Queue a mylist export (done by the servers when load is low, may take up
   * to 24 hours, only one at a time).
   */
  async request_mylist_export(
    template: ExportTemplate,
  ): Promise<"EXPORT_QUEUED"> {
    if (!EXPORT_TEMPLATES.includes(template)) {
      throw illegalInput(
        `${template} is not a valid template. Must be one of: ${EXPORT_TEMPLATES.join(", ")}`,
      );
    }
    await this.callApi("MYLISTEXPORT", { template }, [
      RESPONSE_CODE.EXPORT_QUEUED,
    ]);
    return "EXPORT_QUEUED";
  }

  async cancel_mylist_export(): Promise<"EXPORT_CANCELLED"> {
    await this.callApi("MYLISTEXPORT", { cancel: 1 }, [
      RESPONSE_CODE.EXPORT_CANCELLED,
    ]);
    return "EXPORT_CANCELLED";
  }

  async send_message(
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
      // TODO: phase 3 the old client printed the title length here
      throw illegalInput(
        `body must be shorter than 900 characters but was ${title.length}`,
      );
    }
    await this.callApi("SENDMSG", { username, body, title });
    return "SENDMESSAGE_SUCCESSFUL";
  }

  /** Get a user id by username, or the username by uid */
  get_user_id(username: string): Promise<UserIdResult>;
  get_user_id(uid: number): Promise<UserIdResult>;
  async get_user_id(user: string | number): Promise<UserIdResult> {
    const { data } = await this.callApi(
      "USER",
      nameOrId(user, "uid", "user", {}),
    );
    return USER_PARSER.parseData(data);
  }

  // ---- internals

  /**
   * Send a command. Codes in `expected` are returned as they are, other codes
   * above 305 become an AnidbError. Fatal errors disconnect the client.
   */
  private async callApi(
    command: string,
    params: Params,
    expected: ResponseCode[] = [],
    priority: number = DEFAULT_PRIORITY,
  ): Promise<ParsedResponse> {
    try {
      if (!this.transport.isOpen) {
        throw new AnidbError(RESPONSE_CODE.NOT_CONNECTED);
      }
      const response = await this.transport.request(command, params, priority);
      if ((expected as number[]).includes(response.code)) return response;
      if (response.code > 305) {
        throw new AnidbError(response.code as ResponseCode, response.data);
      }
      return response;
    } catch (_e) {
      const error =
        _e instanceof AnidbError
          ? _e
          : new AnidbError(
              RESPONSE_CODE.UNKNOWN_API_ERROR,
              (_e as Error | undefined)?.message,
            );
      if (FATAL_ERROR_CODES.includes(error.code)) {
        try {
          await this.disconnect(`callApi.${command}`, error);
        } catch (_dc) {
          log(
            `Could not properly disconnect after fatal error: ${(_dc as Error).message}`,
            "warn",
          );
        }
      }
      throw error;
    } finally {
      this.lastRequest = Date.now();
    }
  }

  private async closeConnection(
    source?: unknown,
    error?: Error,
  ): Promise<void> {
    try {
      if (this.transport.session) {
        await this.transport.request("LOGOUT", {}, LOGOUT_PRIORITY);
      }
    } finally {
      clearTimeout(this.pingTimer);
      this.pingTimer = undefined;
      await this.transport.close();
      this.options.onDisconnect?.({ source, error });
    }
  }

  /** ping when nothing was sent for `pingInterval`, so the session stays alive */
  private scheduleKeepAlive(): void {
    clearTimeout(this.pingTimer);
    if (!this.transport.session) return;
    const wait = Math.max(this.lastRequest + this.pingInterval - Date.now(), 0);
    this.pingTimer = setTimeout(() => {
      void this.keepAlive();
    }, wait);
    this.pingTimer.unref();
  }

  private async keepAlive(): Promise<void> {
    if (!this.transport.session) return;
    try {
      if (Date.now() > this.lastRequest + this.pingInterval) {
        await this.ping();
      }
    } catch (_e) {
      try {
        await this.disconnect("autoPing", _e as Error);
      } catch (_dc) {
        log(
          `Could not properly disconnect after failed ping: ${(_dc as Error).message}`,
          "warn",
        );
      }
      return;
    }
    this.scheduleKeepAlive();
  }

  /** the request parameters of the mylist options, undefined ones are skipped */
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
