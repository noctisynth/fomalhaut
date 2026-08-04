import { FomalhautClient } from "fomalhaut-sdk";
import { describe, expect, test } from "vitest";
import { createThemeStore } from "@/state/theme-store";
import { MockTransport, snapshot } from "@/test/mock-transport";

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
      const client = new FomalhautClient(transport);
      const runtime = createThemeStore(client);

      await runtime.initialize();

      expect(runtime.store.getState().screen).toEqual({
        name: "user-selection",
      });
      expect(transport.requests.map((request) => request.method)).toEqual([
        "state.get",
      ]);
    },
  );

  test("skips selection and starts PAM for exactly one trusted user", async () => {
    const transport = new MockTransport(snapshot([alice]));
    const client = new FomalhautClient(transport);
    const runtime = createThemeStore(client);

    await runtime.initialize();

    expect(runtime.store.getState().screen).toEqual({
      name: "known-user",
      user: alice,
    });
    expect(transport.requests.map((request) => request.method)).toEqual([
      "state.get",
      "auth.begin",
    ]);
  });

  test("starts authentication only after a known user is chosen", async () => {
    const transport = new MockTransport(snapshot([alice, bob]));
    const client = new FomalhautClient(transport);
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
    const client = new FomalhautClient(transport);
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
    const client = new FomalhautClient(transport);
    const runtime = createThemeStore(client);

    await runtime.initialize();

    expect(runtime.store.getState().screen).toEqual({
      name: "authentication-recovery",
    });
  });
});

test("converts protocol events into recovered snapshot state", async () => {
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
  transport.emit({
    protocol: 1,
    sequence: 4,
    event: "session.selected",
    data: { sessionId: "x11" },
  });

  expect(runtime.store.getState().snapshot).toMatchObject({
    authentication: "waiting_for_prompt",
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
  expect(runtime.store.getState().snapshot?.authentication).toBe("started");
});

test("cancels an active authentication before returning", async () => {
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

  expect(await runtime.store.getState().cancelAndReturn()).toBe(true);
  expect(transport.requests.at(-1)?.method).toBe("auth.cancel");
  expect(runtime.store.getState().screen.name).toBe("user-selection");
});

test("does not leave authentication when cancellation fails", async () => {
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
