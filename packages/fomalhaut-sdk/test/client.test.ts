import { describe, expect, test } from "bun:test";
import {
  type AnyFomalhautClient,
  createFomalhautClient,
  FomalhautBridgeError,
  FomalhautBusyError,
  type FomalhautClient,
  FomalhautProtocolError,
  type FomalhautTransport,
  type RequestEnvelope,
  type StateSnapshotFor,
} from "../src/index.js";

const GREETER_STATE: StateSnapshotFor<"greeter"> = {
  mode: "greeter",
  authentication: "idle",
  login: "idle",
  prompt: null,
  messages: [],
  sequence: 0,
  users: [],
  sessions: [],
  selectedSessionId: null,
  capabilities: { power: [] },
};

const LOCKER_STATE: StateSnapshotFor<"locker"> = {
  mode: "locker",
  authentication: "idle",
  lock: "locked",
  prompt: null,
  messages: [],
  sequence: 0,
  identity: {
    username: "alice",
    displayName: "Alice",
    avatarUrl: null,
  },
  capabilities: { power: [] },
};

class MockTransport implements FomalhautTransport {
  public readonly requests: RequestEnvelope[] = [];
  public snapshot: StateSnapshotFor<"greeter"> | StateSnapshotFor<"locker">;
  public handler: (request: RequestEnvelope) => Promise<unknown>;
  #receiver: ((event: unknown) => void) | undefined;

  public constructor(
    snapshot:
      | StateSnapshotFor<"greeter">
      | StateSnapshotFor<"locker"> = GREETER_STATE,
  ) {
    this.snapshot = snapshot;
    this.handler = async (request) => ({
      protocol: 1,
      id: request.id,
      ok: true,
      result: request.method === "state.get" ? this.snapshot : {},
    });
  }

  public request(request: RequestEnvelope): Promise<unknown> {
    this.requests.push(request);
    return this.handler(request);
  }

  public subscribe(receiver: (event: unknown) => void): () => void {
    this.#receiver = receiver;
    return () => {
      this.#receiver = undefined;
    };
  }

  public emit(event: unknown): void {
    this.#receiver?.(event);
  }
}

async function connectGreeter(
  transport: MockTransport = new MockTransport(),
): Promise<FomalhautClient<"greeter">> {
  const client = await createFomalhautClient(transport);
  if (client.mode !== "greeter") {
    throw new Error("the greeter fixture returned a locker client");
  }
  return client;
}

const verifyModeNarrowing = (client: AnyFomalhautClient): void => {
  if (client.mode === "greeter") {
    void client.auth.begin("alice");
    void client.session.select("wayland:sway");
    client.on("session.started", () => undefined);
    // @ts-expect-error greeter auth.begin requires a username
    void client.auth.begin();
    // @ts-expect-error greeter clients cannot subscribe to lock events
    client.on("lock.acquired", () => undefined);
  } else {
    void client.auth.begin();
    client.on("lock.acquired", () => undefined);
    // @ts-expect-error locker auth.begin does not accept a username
    void client.auth.begin("alice");
    // @ts-expect-error locker session is statically undefined
    void client.session.select("wayland:sway");
    // @ts-expect-error locker clients cannot subscribe to session events
    client.on("session.started", () => undefined);
  }
};

describe("FomalhautClient", () => {
  test("bootstraps mode and builds correlated requests", async () => {
    const transport = new MockTransport();
    const client = await connectGreeter(transport);

    expect(client.mode).toBe("greeter");
    await expect(client.state.get()).resolves.toEqual(GREETER_STATE);
    await expect(client.auth.begin("alice")).resolves.toBeUndefined();
    await expect(
      client.session.select("wayland:sway"),
    ).resolves.toBeUndefined();
    expect(transport.requests).toEqual([
      { protocol: 1, id: 1, method: "state.get", params: {} },
      { protocol: 1, id: 2, method: "state.get", params: {} },
      {
        protocol: 1,
        id: 3,
        method: "auth.begin",
        params: { username: "alice" },
      },
      {
        protocol: 1,
        id: 4,
        method: "session.select",
        params: { sessionId: "wayland:sway" },
      },
    ]);
  });

  test("uses a parameterless locker auth facade with no session API", async () => {
    const transport = new MockTransport(LOCKER_STATE);
    const client = await createFomalhautClient(transport);
    if (client.mode !== "locker") {
      throw new Error("the locker fixture returned a greeter client");
    }

    expect(client.session).toBeUndefined();
    await expect(client.auth.begin()).resolves.toBeUndefined();
    await expect(client.power.request("suspend")).resolves.toBeUndefined();
    expect(transport.requests.slice(1)).toEqual([
      { protocol: 1, id: 2, method: "auth.begin", params: {} },
      {
        protocol: 1,
        id: 3,
        method: "power.request",
        params: { action: "suspend" },
      },
    ]);
  });

  test("exposes sanitized protocol rejections", async () => {
    const transport = new MockTransport();
    const client = await connectGreeter(transport);
    transport.handler = async (request) => ({
      protocol: 1,
      id: request.id,
      ok: false,
      error: {
        code: "invalid_state",
        message: "authentication is unavailable",
        retryable: false,
      },
    });

    const pending = client.auth.begin("alice");
    await expect(pending).rejects.toBeInstanceOf(FomalhautProtocolError);
    await expect(pending).rejects.toMatchObject({
      requestId: 2,
      body: { code: "invalid_state", retryable: false },
    });
  });

  test("wraps bootstrap, transport, correlation, and mode failures", async () => {
    const malformed = new MockTransport();
    malformed.handler = async (request) => ({
      protocol: 1,
      id: request.id,
      ok: true,
      result: {},
    });
    await expect(createFomalhautClient(malformed)).rejects.toBeInstanceOf(
      FomalhautBridgeError,
    );

    const transport = new MockTransport();
    const client = await connectGreeter(transport);
    transport.handler = async () => {
      throw new Error("native failure");
    };
    await expect(client.auth.cancel()).rejects.toBeInstanceOf(
      FomalhautBridgeError,
    );

    transport.handler = async (request) => ({
      protocol: 1,
      id: request.id + 1,
      ok: true,
      result: {},
    });
    await expect(client.auth.cancel()).rejects.toBeInstanceOf(
      FomalhautBridgeError,
    );

    transport.handler = async (request) => ({
      protocol: 1,
      id: request.id,
      ok: true,
      result: LOCKER_STATE,
    });
    await expect(client.state.get()).rejects.toBeInstanceOf(
      FomalhautBridgeError,
    );
  });

  test("rejects concurrent requests without queueing them", async () => {
    const transport = new MockTransport();
    const client = await connectGreeter(transport);
    let finish: ((value: unknown) => void) | undefined;
    transport.handler = (request) =>
      new Promise((resolve) => {
        finish = (result) =>
          resolve({ protocol: 1, id: request.id, ok: true, result });
      });

    const first = client.auth.begin("alice");
    await expect(client.auth.cancel()).rejects.toBeInstanceOf(
      FomalhautBusyError,
    );
    expect(transport.requests).toHaveLength(2);
    finish?.({});
    await expect(first).resolves.toBeUndefined();
  });

  test("uses the bootstrap watermark and drops wrong-role or out-of-order events", async () => {
    const transport = new MockTransport({ ...GREETER_STATE, sequence: 4 });
    const client = await connectGreeter(transport);
    const messages: string[] = [];
    const unsubscribe = client.on("auth.message", (message, envelope) => {
      messages.push(`${envelope.sequence}:${message.text}`);
    });

    transport.emit({
      protocol: 1,
      sequence: 4,
      event: "auth.message",
      data: { level: "info", text: "at watermark" },
    });
    transport.emit({
      protocol: 1,
      sequence: 5,
      event: "lock.acquired",
      data: {},
    });
    transport.emit({
      protocol: 1,
      sequence: 5,
      event: "auth.message",
      data: { level: "info", text: "first" },
    });
    transport.emit({
      protocol: 1,
      sequence: 5,
      event: "auth.message",
      data: { level: "info", text: "duplicate" },
    });
    transport.emit({
      protocol: 1,
      sequence: 3,
      event: "auth.message",
      data: { level: "info", text: "old" },
    });
    transport.emit({
      protocol: 2,
      sequence: 6,
      event: "auth.message",
      data: { level: "info", text: "wrong version" },
    });

    expect(messages).toEqual(["5:first"]);
    unsubscribe();
    client.close();
  });

  test("generic mode narrowing removes cross-role calls at compile time", async () => {
    const client: AnyFomalhautClient = await createFomalhautClient(
      new MockTransport(),
    );

    expect(client.mode).toBe("greeter");
    expect(verifyModeNarrowing).toBeFunction();
  });
});
