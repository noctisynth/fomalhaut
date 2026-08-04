import { render, screen } from "@testing-library/react";
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

describe("authentication UI", () => {
  test("shows a single user as selected without starting authentication", async () => {
    const transport = new MockTransport(
      snapshot([{ username: "alice", displayName: "Alice", avatarUrl: null }]),
    );

    await renderTheme(transport);

    expect(screen.getByRole("button", { name: /Alice/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(transport.requests.map((request) => request.method)).toEqual([
      "state.get",
    ]);
  });

  test("clears a secret answer before the asynchronous bridge request completes", async () => {
    const transport = new MockTransport(
      snapshot([], { promptId: 7, kind: "secret", message: "Password" }),
    );
    transport.respondPromise = new Promise(() => undefined);
    await renderTheme(transport);
    const user = userEvent.setup();
    const input = screen.getByLabelText("Password");

    await user.type(input, "do-not-retain{Enter}");

    expect(input).toHaveValue("");
    expect(
      screen.getByRole("button", { name: "Authenticating…" }),
    ).toBeDisabled();
  });

  test("keeps multiple users unselected and exposes manual login", async () => {
    const transport = new MockTransport(
      snapshot([
        { username: "alice", displayName: "Alice", avatarUrl: null },
        { username: "bob", displayName: "Bob", avatarUrl: null },
      ]),
    );
    await renderTheme(transport);
    const user = userEvent.setup();

    expect(screen.getByRole("button", { name: /Alice/ })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(screen.getByRole("button", { name: /Bob/ })).toHaveAttribute(
      "aria-pressed",
      "false",
    );

    await user.click(screen.getByRole("button", { name: "Other user" }));
    expect(screen.getByLabelText("Username")).toBeEnabled();
  });

  test("renders visible prompts as text inputs", async () => {
    const transport = new MockTransport(
      snapshot([], { promptId: 8, kind: "visible", message: "One-time code" }),
    );
    await renderTheme(transport);

    expect(screen.getByLabelText("One-time code")).toHaveAttribute(
      "type",
      "text",
    );
  });

  test("uses a non-personal fallback when a display name is empty", async () => {
    const transport = new MockTransport(
      snapshot([{ username: "alice", displayName: "", avatarUrl: null }]),
    );
    await renderTheme(transport);

    expect(screen.getByText("?")).toBeVisible();
  });
});
