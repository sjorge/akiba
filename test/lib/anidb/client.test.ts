import { expect, test } from "bun:test";

import type { ParsedResponse } from "lib/anidb/types";

import { AnidbUDPClient } from "lib/anidb/client";
import { RESPONSE_CODE } from "lib/anidb/constants";
import { DecryptError } from "lib/anidb/udp/transport";

type Step = ParsedResponse | Error;
type Call = {
  command: string;
  // what the transport would put on the wire for this request
  session: string | null;
  encrypted: boolean;
};

const API_KEY = "fake-api-key";
const SALT = "fakesalt";

function reply(code: number, response: string[] = [], data = ""): Step {
  return { code, response, data: data || response.join(" ") };
}

/*
 * A transport that answers from a script, no sockets: the session and key state
 * is recorded as the real transport would see it when sending.
 */
class FakeTransport {
  public session: string | null = null;
  public encryptionKey: Buffer | null = null;
  public isOpen = false;
  public localPort = 4242;
  public calls: Call[] = [];
  public script: Array<[string, Step]> = [];

  public open(): Promise<void> {
    this.isOpen = true;
    return Promise.resolve();
  }

  public close(): Promise<void> {
    this.isOpen = false;
    this.session = null;
    this.encryptionKey = null;
    return Promise.resolve();
  }

  public request(command: string): Promise<ParsedResponse> {
    this.calls.push({
      command,
      session: this.session,
      encrypted: this.encryptionKey !== null,
    });
    const next = this.script.shift();
    if (!next || next[0] !== command) {
      return Promise.reject(
        new Error(`unexpected ${command}, wanted ${next?.[0]}`),
      );
    }
    return typeof next[1] === "object" && "code" in next[1]
      ? Promise.resolve(next[1])
      : Promise.reject(next[1]);
  }
}

function makeClient(warnings: string[] = []): {
  client: AnidbUDPClient;
  transport: FakeTransport;
} {
  const client = new AnidbUDPClient("testclient", {
    onWarning: (message: string): void => {
      warnings.push(message);
    },
  });
  const transport = new FakeTransport();
  (client as unknown as { transport: FakeTransport }).transport = transport;
  return { client, transport };
}

const pong = reply(RESPONSE_CODE.PONG);

function sequence(transport: FakeTransport): string[] {
  return transport.calls.map(
    (c) =>
      `${c.command}${c.session ? " s" : ""}${c.encrypted ? " enc" : " plain"}`,
  );
}

async function connectEncrypted(
  client: AnidbUDPClient,
  transport: FakeTransport,
): Promise<void> {
  transport.script.push(
    ["ENCRYPT", reply(RESPONSE_CODE.ENCRYPTION_ENABLED, [SALT])],
    ["AUTH", reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess1", "1.2.3.4:4242"])],
  );
  await client.connect("fakeuser", "fakepass", API_KEY);
  transport.calls = [];
}

test("the client name warning waits for the first connect and shows once", async (): Promise<void> => {
  const warnings: string[] = [];
  const client = new AnidbUDPClient("Bad_Name", {
    onWarning: (message: string): void => {
      warnings.push(message);
    },
  });
  expect(warnings).toEqual([]);

  const transport = new FakeTransport();
  (client as unknown as { transport: FakeTransport }).transport = transport;
  transport.script.push(
    ["AUTH", reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess1", "1.2.3.4:4242"])],
    ["LOGOUT", reply(RESPONSE_CODE.LOGGED_OUT)],
    ["AUTH", reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess2", "1.2.3.4:4242"])],
    ["LOGOUT", reply(RESPONSE_CODE.LOGGED_OUT)],
  );
  await client.connect("fakeuser", "fakepass");
  expect(warnings).toHaveLength(1);
  expect(warnings[0]).toContain('"Bad_Name"');
  await client.connect("fakeuser", "fakepass");
  expect(warnings).toHaveLength(1);
  await client.disconnect();
});

test("a valid client name never warns", async (): Promise<void> => {
  const warnings: string[] = [];
  const { client, transport } = makeClient(warnings);
  transport.script.push([
    "AUTH",
    reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess1", "1.2.3.4:4242"]),
  ]);
  await client.connect("fakeuser", "fakepass");
  expect(warnings).toEqual([]);
  await client.disconnect().catch(() => {});
});

test("after a 501 the login starts over: ENCRYPT plain, AUTH encrypted, command once more", async (): Promise<void> => {
  const warnings: string[] = [];
  const { client, transport } = makeClient(warnings);
  await connectEncrypted(client, transport);

  transport.script.push(
    ["PING", reply(RESPONSE_CODE.LOGIN_FIRST)],
    ["ENCRYPT", reply(RESPONSE_CODE.ENCRYPTION_ENABLED, [SALT])],
    ["AUTH", reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess2", "1.2.3.4:4242"])],
    ["PING", pong],
  );
  await client.ping();
  expect(sequence(transport)).toEqual([
    "PING s enc",
    "ENCRYPT plain",
    "AUTH enc",
    "PING s enc",
  ]);
  await client.disconnect().catch(() => {});
});

test("after a 506 the login starts over the same way", async (): Promise<void> => {
  const { client, transport } = makeClient();
  await connectEncrypted(client, transport);

  transport.script.push(
    ["PING", reply(RESPONSE_CODE.INVALID_SESSION)],
    ["ENCRYPT", reply(RESPONSE_CODE.ENCRYPTION_ENABLED, [SALT])],
    ["AUTH", reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess2", "1.2.3.4:4242"])],
    ["PING", pong],
  );
  await client.ping();
  expect(sequence(transport)).toEqual([
    "PING s enc",
    "ENCRYPT plain",
    "AUTH enc",
    "PING s enc",
  ]);
  await client.disconnect().catch(() => {});
});

test("a reply that cannot be decrypted counts as a lost session", async (): Promise<void> => {
  const { client, transport } = makeClient();
  await connectEncrypted(client, transport);

  transport.script.push(
    ["PING", new DecryptError("bad padding")],
    ["ENCRYPT", reply(RESPONSE_CODE.ENCRYPTION_ENABLED, [SALT])],
    ["AUTH", reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess2", "1.2.3.4:4242"])],
    ["PING", pong],
  );
  await client.ping();
  expect(sequence(transport)).toEqual([
    "PING s enc",
    "ENCRYPT plain",
    "AUTH enc",
    "PING s enc",
  ]);
  await client.disconnect().catch(() => {});
});

test("without an api key a lost session is just AUTH then the command", async (): Promise<void> => {
  const { client, transport } = makeClient();
  transport.script.push([
    "AUTH",
    reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess1", "1.2.3.4:4242"]),
  ]);
  await client.connect("fakeuser", "fakepass");
  transport.calls = [];

  transport.script.push(
    ["PING", reply(RESPONSE_CODE.LOGIN_FIRST)],
    ["AUTH", reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess2", "1.2.3.4:4242"])],
    ["PING", pong],
  );
  await client.ping();
  expect(sequence(transport)).toEqual([
    "PING s plain",
    "AUTH plain",
    "PING s plain",
  ]);
  await client.disconnect().catch(() => {});
});

test("the retry happens once: a second 501 is reported", async (): Promise<void> => {
  const { client, transport } = makeClient();
  await connectEncrypted(client, transport);

  transport.script.push(
    ["PING", reply(RESPONSE_CODE.LOGIN_FIRST)],
    ["ENCRYPT", reply(RESPONSE_CODE.ENCRYPTION_ENABLED, [SALT])],
    ["AUTH", reply(RESPONSE_CODE.LOGIN_ACCEPTED, ["sess2", "1.2.3.4:4242"])],
    ["PING", reply(RESPONSE_CODE.LOGIN_FIRST)],
  );
  const error = await client.ping().catch((e: unknown) => e);
  expect((error as { code: number }).code).toBe(RESPONSE_CODE.LOGIN_FIRST);
  expect(transport.calls.filter((c) => c.command === "PING").length).toBe(2);
});
