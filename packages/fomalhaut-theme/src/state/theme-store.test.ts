import { FomalhautClient } from "fomalhaut-sdk";
import { describe, expect, test } from "vitest";
import { createThemeStore, initialUsername } from "@/state/theme-store";
import { MockTransport, snapshot } from "@/test/mock-transport";

const alice = {
  username: "alice",
  displayName: "Alice",
  avatarUrl: null,
};
const bob = { username: "bob", displayName: "Bob", avatarUrl: null };

describe("initial user selection", () => {
  test("selects exactly one discovered user", () => {
    expect(initialUsername([alice])).toBe("alice");
  });

  test("does not select zero or multiple users", () => {
    expect(initialUsername([])).toBeNull();
    expect(initialUsername([alice, bob])).toBeNull();
  });

  test.each([
    { users: [], username: null, manual: true },
    { users: [alice], username: "alice", manual: false },
    { users: [alice, bob], username: null, manual: false },
  ])(
    "restores $users.length user summaries",
    async ({ users, username, manual }) => {
      const transport = new MockTransport(snapshot(users));
      const client = new FomalhautClient(transport);
      const runtime = createThemeStore(client);

      await runtime.initialize();

      expect(runtime.store.getState().selectedUsername).toBe(username);
      expect(runtime.store.getState().manualUsername).toBe(manual);
      expect(transport.requests).toHaveLength(1);
      runtime.destroy();
      client.close();
    },
  );
});

test("keeps an explicit choice across initial state recovery", async () => {
  const transport = new MockTransport(snapshot([alice]));
  const client = new FomalhautClient(transport);
  const runtime = createThemeStore(client);
  runtime.store.getState().selectOtherUser();

  await runtime.initialize();

  expect(runtime.store.getState().manualUsername).toBe(true);
  expect(runtime.store.getState().selectedUsername).toBeNull();
});

test("converts session selection events into snapshot state", async () => {
  const transport = new MockTransport(snapshot([alice]));
  const client = new FomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();

  transport.emit({
    protocol: 1,
    sequence: 1,
    event: "session.selected",
    data: { sessionId: "x11" },
  });

  expect(runtime.store.getState().snapshot?.selectedSessionId).toBe("x11");
});

test("converts authentication events without retaining prompt answers", async () => {
  const transport = new MockTransport(snapshot([alice]));
  const client = new FomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();

  transport.emit({
    protocol: 1,
    sequence: 1,
    event: "state.changed",
    data: { state: "waiting_for_prompt" },
  });
  transport.emit({
    protocol: 1,
    sequence: 2,
    event: "auth.prompt",
    data: { promptId: 4, kind: "visible", message: "Token" },
  });
  transport.emit({
    protocol: 1,
    sequence: 3,
    event: "auth.message",
    data: { level: "info", text: "Touch your security key" },
  });

  expect(runtime.store.getState().snapshot).toMatchObject({
    authentication: "waiting_for_prompt",
    prompt: { promptId: 4, kind: "visible", message: "Token" },
    messages: [{ level: "info", text: "Touch your security key" }],
  });

  transport.emit({
    protocol: 1,
    sequence: 4,
    event: "auth.cancelled",
    data: {},
  });
  expect(runtime.store.getState().snapshot?.prompt).toBeNull();

  transport.emit({
    protocol: 1,
    sequence: 5,
    event: "session.started",
    data: {},
  });
  expect(runtime.store.getState().snapshot?.authentication).toBe("started");
});

test("applies busy backpressure before requests reach the SDK", async () => {
  const transport = new MockTransport(snapshot([alice]));
  transport.respondPromise = new Promise(() => undefined);
  const client = new FomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();
  const prompt = { promptId: 1, kind: "secret", message: "Password" } as const;

  void runtime.store.getState().respondToPrompt(prompt, "first");
  await Promise.resolve();
  void runtime.store.getState().selectSession("x11");

  expect(transport.requests.map((request) => request.method)).toEqual([
    "state.get",
    "auth.respond",
  ]);
});
