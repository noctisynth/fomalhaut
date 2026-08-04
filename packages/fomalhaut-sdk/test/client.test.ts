import { describe, expect, test } from "bun:test";
import {
  FomalhautBridgeError,
  FomalhautBusyError,
  FomalhautClient,
  FomalhautProtocolError,
  type FomalhautTransport,
  type RequestEnvelope,
  type StateSnapshot,
} from "../src/index.js";

const EMPTY_STATE: StateSnapshot = {
  authentication: "idle",
  prompt: null,
  messages: [],
  users: [],
  sessions: [],
  selectedSessionId: null,
  capabilities: { power: [] },
};

class MockTransport implements FomalhautTransport {
  public readonly requests: RequestEnvelope[] = [];
  public handler: (request: RequestEnvelope) => Promise<unknown> = async (
    request,
  ) => ({
    protocol: 1,
    id: request.id,
    ok: true,
    result: {},
  });
  #receiver: ((event: unknown) => void) | undefined;

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

describe("FomalhautClient", () => {
  test("builds correlated protocol requests and returns state", async () => {
    const transport = new MockTransport();
    transport.handler = async (request) => ({
      protocol: 1,
      id: request.id,
      ok: true,
      result: EMPTY_STATE,
    });
    const client = new FomalhautClient(transport);

    await expect(client.state.get()).resolves.toEqual(EMPTY_STATE);
    expect(transport.requests).toEqual([
      { protocol: 1, id: 1, method: "state.get", params: {} },
    ]);
  });

  test("exposes sanitized protocol rejections", async () => {
    const transport = new MockTransport();
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
    const client = new FomalhautClient(transport);

    const pending = client.auth.begin("alice");
    await expect(pending).rejects.toBeInstanceOf(FomalhautProtocolError);
    await expect(pending).rejects.toMatchObject({
      requestId: 1,
      body: { code: "invalid_state", retryable: false },
    });
  });

  test("sends an enumerated power request", async () => {
    const transport = new MockTransport();
    const client = new FomalhautClient(transport);

    await expect(client.power.request("suspend")).resolves.toBeUndefined();
    expect(transport.requests).toEqual([
      {
        protocol: 1,
        id: 1,
        method: "power.request",
        params: { action: "suspend" },
      },
    ]);
  });

  test("wraps transport and response-correlation failures", async () => {
    const transport = new MockTransport();
    transport.handler = async () => {
      throw new Error("native failure");
    };
    const client = new FomalhautClient(transport);
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
  });

  test("rejects concurrent requests without queueing them", async () => {
    const transport = new MockTransport();
    let finish: ((value: unknown) => void) | undefined;
    transport.handler = (request) =>
      new Promise((resolve) => {
        finish = (result) =>
          resolve({ protocol: 1, id: request.id, ok: true, result });
      });
    const client = new FomalhautClient(transport);

    const first = client.auth.begin("alice");
    await expect(client.auth.cancel()).rejects.toBeInstanceOf(
      FomalhautBusyError,
    );
    expect(transport.requests).toHaveLength(1);
    finish?.({});
    await expect(first).resolves.toBeUndefined();
  });

  test("narrows events and drops repeated or out-of-order sequences", () => {
    const transport = new MockTransport();
    const client = new FomalhautClient(transport);
    const messages: string[] = [];
    const unsubscribe = client.on("auth.message", (message) => {
      messages.push(message.text);
    });

    transport.emit({
      protocol: 1,
      sequence: 2,
      event: "auth.message",
      data: { level: "info", text: "first" },
    });
    transport.emit({
      protocol: 1,
      sequence: 2,
      event: "auth.message",
      data: { level: "info", text: "duplicate" },
    });
    transport.emit({
      protocol: 1,
      sequence: 1,
      event: "auth.message",
      data: { level: "info", text: "old" },
    });
    transport.emit({
      protocol: 2,
      sequence: 3,
      event: "auth.message",
      data: { level: "info", text: "wrong version" },
    });

    expect(messages).toEqual(["first"]);
    unsubscribe();
    client.close();
  });
});
