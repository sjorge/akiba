/*
 * UDP socket of the AniDB API: one request in flight, matched by tag
 */
import dgram from "node:dgram";

import type { ParsedResponse } from "lib/anidb/types";

import {
  ANIDB_HOST,
  ANIDB_PORT,
  REQUEST_INTERVAL_MS,
  REQUEST_TIMEOUT_MS,
  RESPONSE_CODE,
} from "lib/anidb/constants";
import { AnidbError } from "lib/anidb/error";
import { bytes } from "lib/anidb/udp/bytes";
import { decrypt, encrypt } from "lib/anidb/udp/crypto";
import { decompress, isCompressed } from "lib/anidb/udp/compression";
import { encodeRequest, makeTag, parseResponse } from "lib/anidb/udp/protocol";
import { RateLimiter } from "lib/anidb/udp/rate-limiter";

export type UdpTransportOptions = {
  host?: string;
  port?: number;
  // minimum time between two request starts in milliseconds
  interval?: number;
  // time to wait for a reply in milliseconds
  timeout?: number;
  // called with problems that do not stop the transport
  onWarning?: (message: string) => void;
};

type Pending = {
  resolve: (response: ParsedResponse) => void;
  reject: (reason: unknown) => void;
  timer: ReturnType<typeof setTimeout>;
};

const DEFAULT_PRIORITY: number = 5;
// replies without a tag: 504 and 555 (bans) and 6xx (server errors)
const UNTAGGED_ERROR: RegExp = /^(504|555|6\d\d) /;

/*
 * One UDP socket and the request/reply matching on top of it. Every command is
 * one datagram, replies are matched by tag and are at most 1400 bytes (the
 * default MTU of the spec, 400 at the least). Requests go through a rate
 * limiter that keeps to the flood protection of the spec, so only one request
 * is in flight at a time.
 */
export class UdpTransport {
  // session key, sent as `s` with every request when set
  public session: string | null = null;
  // when set all requests are encrypted and all replies decrypted
  public encryptionKey: Buffer | null = null;

  private socket: dgram.Socket | null = null;
  private limiter: RateLimiter | null = null;
  private pending: Map<string, Pending> = new Map();
  private host: string;
  private port: number;
  private interval: number;
  private timeout: number;
  private onWarning: ((message: string) => void) | undefined;

  public constructor(options: UdpTransportOptions = {}) {
    this.host = options.host ?? ANIDB_HOST;
    this.port = options.port ?? ANIDB_PORT;
    this.interval = options.interval ?? REQUEST_INTERVAL_MS;
    this.timeout = options.timeout ?? REQUEST_TIMEOUT_MS;
    this.onWarning = options.onWarning;
  }

  public toString(): string {
    return "UdpTransport";
  }

  public get isOpen(): boolean {
    return this.socket !== null;
  }

  /*
   * the port the local socket is bound to, null when closed
   */
  public get localPort(): number | null {
    if (this.socket === null) return null;
    try {
      return this.socket.address().port;
    } catch {
      // not bound (anymore)
      return null;
    }
  }

  /*
   * bind the local socket, a fixed `localPort` is recommended by the spec
   */
  public async open(localPort?: number | null): Promise<void> {
    const socket = dgram.createSocket("udp4");
    try {
      await new Promise<void>((resolve, reject) => {
        socket.once("error", reject);
        socket.bind({ port: localPort ?? undefined }, () => {
          socket.removeListener("error", reject);
          resolve();
        });
      });
    } catch (_e: unknown) {
      socket.close();
      throw _e;
    }
    socket.on("message", (message) => {
      void this.handleMessage(message);
    });
    socket.on("error", (error) => {
      this.rejectAll(error);
    });
    socket.on("close", () => {
      this.rejectAll(new AnidbError(RESPONSE_CODE.NOT_CONNECTED));
    });
    this.socket = socket;
    this.limiter = new RateLimiter(this.interval);
  }

  /*
   * drop waiting requests, close the socket and forget session and key
   */
  public async close(): Promise<void> {
    const socket = this.socket;
    this.limiter?.stop();
    this.limiter = null;
    this.socket = null;
    this.session = null;
    this.encryptionKey = null;
    if (socket) {
      await new Promise<void>((resolve) => {
        socket.close(() => {
          resolve();
        });
      });
    }
    this.rejectAll(new AnidbError(RESPONSE_CODE.NOT_CONNECTED));
  }

  /*
   * lower priority numbers are sent first
   */
  public request(
    command: string,
    params: Record<string, string | number> = {},
    priority: number = DEFAULT_PRIORITY,
  ): Promise<ParsedResponse> {
    const limiter = this.limiter;
    if (limiter === null) {
      return Promise.reject(new AnidbError(RESPONSE_CODE.NOT_CONNECTED));
    }
    return limiter.schedule(priority, () => this.send(command, params));
  }

  private send(
    command: string,
    params: Record<string, string | number>,
  ): Promise<ParsedResponse> {
    const socket = this.socket;
    if (socket === null) {
      return Promise.reject(new AnidbError(RESPONSE_CODE.NOT_CONNECTED));
    }

    const tag = makeTag();
    const requestParams: Record<string, string | number> = { ...params, tag };
    if (this.session) requestParams.s = this.session;

    let payload = Buffer.from(encodeRequest(command, requestParams));
    if (this.encryptionKey) payload = encrypt(payload, this.encryptionKey);

    return new Promise<ParsedResponse>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(tag);
        reject(
          new AnidbError(
            RESPONSE_CODE.CONNECTION_TIMEOUT,
            `no reply from ${this.host} within ${this.timeout / 1000}s`,
          ),
        );
      }, this.timeout);
      this.pending.set(tag, { resolve, reject, timer });
      socket.send(bytes(payload), this.port, this.host, (error) => {
        if (error) this.settle(tag, error);
      });
    });
  }

  /*
   * A reply may be encrypted (after ENCRYPT) and compressed (two zero bytes
   * first, after `comp=1`). The tag in front of it says which request it
   * answers. Server errors (6xx) and bans (504, 555) come without a tag, since
   * only one request is in flight they are given to that one.
   */
  private async handleMessage(message: Buffer): Promise<void> {
    let data = message;
    try {
      if (this.encryptionKey) {
        try {
          data = decrypt(data, this.encryptionKey);
        } catch (_e: unknown) {
          // most likely anidb dropped encryption, so stop using it
          this.encryptionKey = null;
          this.onWarning?.(
            "AniDB sent a reply that cannot be decrypted, encryption is turned off for the rest of this connection",
          );
          throw new Error(`Message decrypt failed: ${(_e as Error).message}`);
        }
      }
      if (isCompressed(data)) data = decompress(data);

      const text = data.toString();
      const tag = text.slice(0, 5);
      const pending = this.pending.get(tag);
      if (pending === undefined || text[5] !== " ") {
        // server errors (6xx), bans (504, 555) come without a tag, only one
        // request is ever in flight so the reply belongs to it
        if (UNTAGGED_ERROR.test(text)) {
          const response = parseResponse(text);
          for (const pendingTag of this.pending.keys()) {
            this.settle(pendingTag, undefined, response);
          }
        }
        return;
      }

      this.settle(tag, undefined, parseResponse(text.slice(6)));
    } catch (_e: unknown) {
      // a reply we cannot read belongs to the request in flight
      for (const tag of this.pending.keys()) this.settle(tag, _e);
    }
  }

  private settle(
    tag: string,
    error?: unknown,
    response?: ParsedResponse,
  ): void {
    const pending = this.pending.get(tag);
    if (pending === undefined) return;
    clearTimeout(pending.timer);
    this.pending.delete(tag);
    if (error !== undefined || response === undefined) pending.reject(error);
    else pending.resolve(response);
  }

  private rejectAll(error: unknown): void {
    for (const tag of [...this.pending.keys()]) this.settle(tag, error);
  }
}

// vim: tabstop=2 shiftwidth=2 softtabstop=0 smarttab expandtab
