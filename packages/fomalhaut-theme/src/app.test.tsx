import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FomalhautClient } from "fomalhaut-sdk";
import { describe, expect, test } from "vitest";
import { App } from "@/app";
import { createThemeStore } from "@/state/theme-store";
import { ThemeStoreProvider } from "@/state/theme-store-provider";
import { MockTransport, snapshot } from "@/test/mock-transport";

async function renderTheme(transport: MockTransport) {
  const client = new FomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();
  render(
    <ThemeStoreProvider store={runtime.store}>
      <App />
    </ThemeStoreProvider>,
  );
  return { runtime, client };
}

describe("SPA authentication UI", () => {
  test("skips selection and starts authentication for one known user", async () => {
    const transport = new MockTransport(
      snapshot([{ username: "alice", displayName: "Alice", avatarUrl: null }]),
    );
    await renderTheme(transport);

    expect(screen.getByRole("heading", { name: "Alice" })).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "Who’s signing in?" }),
    ).not.toBeInTheDocument();
    expect(transport.requests.map((request) => request.method)).toEqual([
      "state.get",
      "auth.begin",
    ]);
  });

  test("opens known-user authentication after an explicit selection", async () => {
    const transport = new MockTransport(
      snapshot([
        { username: "alice", displayName: "Alice", avatarUrl: null },
        { username: "bob", displayName: "Bob", avatarUrl: null },
      ]),
    );
    await renderTheme(transport);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /Alice/ }));

    expect(screen.getByRole("heading", { name: "Alice" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Back to users" })).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Back to users" }).closest("section"),
    ).not.toHaveClass("zoom-in-95");
    expect(transport.requests.at(-1)).toMatchObject({ method: "auth.begin" });
  });

  test("centers all choices on a multi-user selection screen", async () => {
    const transport = new MockTransport(
      snapshot([
        { username: "alice", displayName: "Alice", avatarUrl: null },
        { username: "bob", displayName: "Bob", avatarUrl: null },
      ]),
    );
    await renderTheme(transport);

    expect(screen.getByTestId("account-list")).toHaveClass("justify-center");
    expect(screen.getByRole("button", { name: /Alice/ })).toBeVisible();
    expect(screen.getByRole("button", { name: /Bob/ })).toBeVisible();
    expect(screen.getByRole("button", { name: /Other user/ })).toBeVisible();
  });

  test("shows username and disabled credential regions for another user", async () => {
    const transport = new MockTransport(snapshot());
    await renderTheme(transport);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /Other user/ }));

    expect(screen.getByRole("heading", { name: "Sign in" })).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "Other user" }),
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText("Username")).toBeEnabled();
    expect(screen.getByLabelText("Password")).toBeDisabled();
  });

  test("submits a manual username before enabling PAM prompts", async () => {
    const transport = new MockTransport(snapshot());
    await renderTheme(transport);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Other user/ }));

    await user.type(screen.getByLabelText("Username"), "carol{Enter}");

    expect(screen.getByLabelText("Username")).toHaveValue("carol");
    expect(screen.getByLabelText("Username")).toBeDisabled();
    expect(screen.getByLabelText("Password")).toBeDisabled();
    expect(transport.requests.at(-1)).toMatchObject({
      method: "auth.begin",
      params: { username: "carol" },
    });

    act(() => {
      transport.emit({
        protocol: 1,
        sequence: 1,
        event: "auth.prompt",
        data: { promptId: 9, kind: "secret", message: "Password for carol" },
      });
    });

    expect(screen.getByLabelText("Username")).toHaveValue("carol");
    expect(screen.getByLabelText("Password for carol")).toBeEnabled();
  });

  test("clears a secret answer before its asynchronous request completes", async () => {
    const transport = new MockTransport(
      snapshot([], { promptId: 7, kind: "secret", message: "Password" }),
    );
    transport.respondPromise = new Promise(() => undefined);
    await renderTheme(transport);
    const user = userEvent.setup();
    const input = screen.getByLabelText("Password");

    await user.type(input, "do-not-retain{Enter}");

    expect(input).toHaveValue("");
    expect(screen.getByRole("button", { name: "Sign in" })).toBeDisabled();
  });

  test("renders visible prompts as text inputs on the recovery screen", async () => {
    const transport = new MockTransport(
      snapshot([], { promptId: 8, kind: "visible", message: "One-time code" }),
    );
    await renderTheme(transport);

    expect(
      screen.getByRole("heading", { name: "Authentication" }),
    ).toBeVisible();
    expect(screen.getByLabelText("One-time code")).toHaveAttribute(
      "type",
      "text",
    );
  });

  test("uses a non-personal fallback for an empty display name", async () => {
    const transport = new MockTransport(
      snapshot([
        { username: "alice", displayName: "", avatarUrl: null },
        { username: "bob", displayName: "Bob", avatarUrl: null },
      ]),
    );
    await renderTheme(transport);

    expect(screen.getByText("?")).toBeVisible();
  });

  test("shows only advertised power actions and confirms before requesting", async () => {
    const transport = new MockTransport(
      snapshot([], null, ["reboot", "suspend"]),
    );
    await renderTheme(transport);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Power menu" }));
    expect(
      screen.queryByRole("button", { name: "Power off" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restart" })).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Suspend" }));

    expect(transport.requests.at(-1)?.method).toBe("state.get");
    await user.click(screen.getByRole("button", { name: "Confirm suspend" }));
    expect(transport.requests.at(-1)).toMatchObject({
      method: "power.request",
      params: { action: "suspend" },
    });
  });

  test("retries the same user after authentication failure", async () => {
    const transport = new MockTransport(
      snapshot([
        { username: "alice", displayName: "Alice", avatarUrl: null },
        { username: "bob", displayName: "Bob", avatarUrl: null },
      ]),
    );
    await renderTheme(transport);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Alice/ }));

    act(() => {
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
    });
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(transport.requests.at(-1)).toMatchObject({
      method: "auth.begin",
      params: { username: "alice" },
    });
  });

  test("selects a session through the shadcn select popup", async () => {
    const state = snapshot();
    state.sessions = [
      { id: "wayland", name: "Wayland", kind: "wayland" },
      { id: "x11", name: "X11", kind: "x11" },
    ];
    const transport = new MockTransport(state);
    await renderTheme(transport);
    const user = userEvent.setup();
    const trigger = screen.getByRole("combobox", { name: "Session" });

    expect(trigger).toHaveClass("w-52");
    expect(trigger).toHaveTextContent("Wayland · wayland");

    await user.click(trigger);
    expect(document.querySelector('[data-slot="select-group"]')).not.toBeNull();
    await user.click(screen.getByRole("option", { name: "X11 · x11" }));

    expect(transport.requests.at(-1)).toMatchObject({
      method: "session.select",
      params: { sessionId: "x11" },
    });
  });
});
