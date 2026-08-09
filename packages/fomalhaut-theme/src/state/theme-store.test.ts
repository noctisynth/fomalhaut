import { createFomalhautClient } from "fomalhaut-sdk";
import { describe, expect, test } from "vitest";
import { createThemeStore } from "@/state/theme-store";
import { lockerSnapshot, MockTransport, snapshot } from "@/test/mock-transport";

const alice = {
  username: "alice",
  displayName: "Alice",
  avatarUrl: null,
};
const bob = { username: "bob", displayName: "Bob", avatarUrl: null };

describe("SPA identity selection", () => {
  test.each([{ users: [] }, { users: [alice, bob] }])(
    "always opens the selection screen for $users.length users",
    async ({ users }) => {
      const transport = new MockTransport(snapshot(users));
      const client = await createFomalhautClient(transport);
      const runtime = createThemeStore(client);

      await runtime.initialize();

      expect(runtime.store.getState().screen).toEqual({
        name: "user-selection",
      });
      expect(transport.requests.map((request) => request.method)).toEqual([
        "state.get",
        "state.get",
      ]);
    },
  );

  test("skips selection and starts PAM for exactly one trusted user", async () => {
    const transport = new MockTransport(snapshot([alice]));
    const client = await createFomalhautClient(transport);
    const runtime = createThemeStore(client);

    await runtime.initialize();

    expect(runtime.store.getState().screen).toEqual({
      name: "known-user",
      user: alice,
    });
    expect(transport.requests.map((request) => request.method)).toEqual([
      "state.get",
      "state.get",
      "auth.begin",
    ]);
  });

  test("starts authentication only after a known user is chosen", async () => {
    const transport = new MockTransport(snapshot([alice, bob]));
    const client = await createFomalhautClient(transport);
    const runtime = createThemeStore(client);
    await runtime.initialize();

    await runtime.store.getState().chooseKnownUser(alice);

    expect(runtime.store.getState().screen).toEqual({
      name: "known-user",
      user: alice,
    });
    expect(transport.requests.at(-1)).toMatchObject({
      method: "auth.begin",
      params: { username: "alice" },
    });
  });

  test("keeps manual identity on its authentication screen", async () => {
    const transport = new MockTransport(snapshot());
    const client = await createFomalhautClient(transport);
    const runtime = createThemeStore(client);
    await runtime.initialize();

    runtime.store.getState().chooseOtherUser();
    await runtime.store.getState().submitManualUsername("carol");

    expect(runtime.store.getState().screen).toEqual({
      name: "other-user",
      username: "carol",
    });
    expect(transport.requests.at(-1)).toMatchObject({
      method: "auth.begin",
      params: { username: "carol" },
    });
  });

  test("uses a generic recovery screen when active identity is unavailable", async () => {
    const active = snapshot([], {
      promptId: 3,
      kind: "secret",
      message: "Password",
    });
    const transport = new MockTransport(active);
    const client = await createFomalhautClient(transport);
    const runtime = createThemeStore(client);

    await runtime.initialize();

    expect(runtime.store.getState().screen).toEqual({
      name: "authentication-recovery",
    });
  });
});

test("converts protocol events into recovered snapshot state", async () => {
  const transport = new MockTransport(snapshot([alice]));
  const client = await createFomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();

  transport.emit({
    protocol: 1,
    sequence: 1,
    event: "state.changed",
    data: { state: "waiting_for_visible" },
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
  transport.emit({
    protocol: 1,
    sequence: 4,
    event: "session.selected",
    data: { sessionId: "x11" },
  });

  expect(runtime.store.getState().snapshot).toMatchObject({
    authentication: "waiting_for_visible",
    prompt: { promptId: 4, kind: "visible", message: "Token" },
    messages: [{ level: "info", text: "Touch your security key" }],
    selectedSessionId: "x11",
  });

  transport.emit({
    protocol: 1,
    sequence: 5,
    event: "auth.cancelled",
    data: {},
  });
  expect(runtime.store.getState().snapshot?.prompt).toBeNull();

  transport.emit({
    protocol: 1,
    sequence: 6,
    event: "session.started",
    data: {},
  });
  const started = runtime.store.getState().snapshot;
  expect(started?.mode === "greeter" ? started.login : null).toBe("started");
});

test("cancels an active authentication before returning", async () => {
  const transport = new MockTransport(snapshot([alice]));
  const client = await createFomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();
  transport.emit({
    protocol: 1,
    sequence: 1,
    event: "state.changed",
    data: { state: "waiting_for_secret" },
  });

  expect(await runtime.store.getState().cancelAndReturn()).toBe(true);
  expect(transport.requests.at(-1)?.method).toBe("auth.cancel");
  expect(runtime.store.getState().screen.name).toBe("user-selection");
});

test("does not cancel again after greetd has released a failed attempt", async () => {
  const transport = new MockTransport(snapshot([alice, bob]));
  const client = await createFomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();
  await runtime.store.getState().chooseKnownUser(alice);
  transport.emit({
    protocol: 1,
    sequence: 1,
    event: "state.changed",
    data: { state: "failed" },
  });
  transport.emit({
    protocol: 1,
    sequence: 2,
    event: "auth.failed",
    data: {},
  });
  const requestsBeforeReturn = transport.requests.length;

  expect(await runtime.store.getState().cancelAndReturn()).toBe(true);
  expect(transport.requests).toHaveLength(requestsBeforeReturn);
  expect(runtime.store.getState().screen.name).toBe("user-selection");
});

test("clears PAM failure feedback before retrying the same user", async () => {
  const transport = new MockTransport(snapshot([alice, bob]));
  const client = await createFomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();
  await runtime.store.getState().chooseKnownUser(alice);
  transport.emit({
    protocol: 1,
    sequence: 1,
    event: "auth.message",
    data: {
      level: "error",
      text: "Authenticator could not perform the requested operation",
    },
  });
  transport.emit({
    protocol: 1,
    sequence: 2,
    event: "state.changed",
    data: { state: "failed" },
  });
  transport.emit({
    protocol: 1,
    sequence: 3,
    event: "auth.failed",
    data: {},
  });

  expect(runtime.store.getState().snapshot?.messages).toHaveLength(1);
  expect(await runtime.store.getState().retryAuthentication()).toBe(true);
  expect(runtime.store.getState().snapshot?.messages).toEqual([]);
  expect(runtime.store.getState().error).toBeNull();
  expect(transport.requests.at(-1)).toMatchObject({
    method: "auth.begin",
    params: { username: "alice" },
  });
});

test("does not leave authentication when cancellation fails", async () => {
  const transport = new MockTransport(snapshot([alice]));
  const client = await createFomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();
  transport.emit({
    protocol: 1,
    sequence: 1,
    event: "state.changed",
    data: { state: "waiting_for_secret" },
  });
  transport.rejectMethod = "auth.cancel";

  expect(await runtime.store.getState().cancelAndReturn()).toBe(false);
  expect(runtime.store.getState().screen.name).toBe("known-user");
  expect(runtime.store.getState().error).toBe(
    "The Fomalhaut host is unavailable.",
  );
});

test("applies busy backpressure before requests reach the SDK", async () => {
  const transport = new MockTransport(snapshot([alice, bob]));
  transport.respondPromise = new Promise(() => undefined);
  const client = await createFomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();
  const prompt = { promptId: 1, kind: "secret", message: "Password" } as const;

  void runtime.store.getState().respondToPrompt(prompt, "first");
  await Promise.resolve();
  void runtime.store.getState().selectSession("x11");

  expect(transport.requests.map((request) => request.method)).toEqual([
    "state.get",
    "state.get",
    "auth.respond",
  ]);
});

test("locker mode reauthenticates the fixed identity without session APIs", async () => {
  const transport = new MockTransport(lockerSnapshot());
  const client = await createFomalhautClient(transport);
  const runtime = createThemeStore(client);

  await runtime.initialize();

  expect(runtime.store.getState().screen).toEqual({ name: "locker" });
  expect(runtime.store.getState().snapshot).toMatchObject({
    mode: "locker",
    lock: "locked",
    identity: { username: "alice", displayName: "Alice" },
  });
  expect(transport.requests.map((request) => request.method)).toEqual([
    "state.get",
    "state.get",
    "auth.begin",
  ]);
  expect(transport.requests.at(-1)).toMatchObject({ params: {} });

  transport.emit({
    protocol: 1,
    sequence: 1,
    event: "state.changed",
    data: { state: "waiting_for_secret" },
  });
  transport.emit({
    protocol: 1,
    sequence: 2,
    event: "auth.prompt",
    data: { promptId: 7, kind: "secret", message: "Password" },
  });
  expect(runtime.store.getState().snapshot).toMatchObject({
    mode: "locker",
    authentication: "waiting_for_secret",
    prompt: { promptId: 7, kind: "secret", message: "Password" },
    sequence: 2,
  });

  const requestCount = transport.requests.length;
  expect(await runtime.store.getState().selectSession("x11")).toBe(false);
  expect(transport.requests).toHaveLength(requestCount);

  transport.emit({
    protocol: 1,
    sequence: 3,
    event: "lock.failed",
    data: {},
  });
  expect(runtime.store.getState().snapshot).toMatchObject({
    mode: "locker",
    lock: "failed",
    sequence: 3,
  });
});
