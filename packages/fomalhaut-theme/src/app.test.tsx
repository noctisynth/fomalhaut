import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createFomalhautClient, type FomalhautTransport } from "fomalhaut-sdk";
import { describe, expect, test, vi } from "vitest";
import { App } from "@/app";
import { DevelopmentTransport } from "@/runtime/development-transport";
import { createThemeStore } from "@/state/theme-store";
import { ThemeStoreProvider } from "@/state/theme-store-provider";
import { lockerSnapshot, MockTransport, snapshot } from "@/test/mock-transport";

async function renderTheme(transport: FomalhautTransport) {
  const client = await createFomalhautClient(transport);
  const runtime = createThemeStore(client);
  await runtime.initialize();
  render(
    <ThemeStoreProvider store={runtime.store}>
      <App />
    </ThemeStoreProvider>,
  );
  return { runtime, client };
}

async function openPowerMenu(
  user: ReturnType<typeof userEvent.setup>,
  name: "Power menu" | "电源菜单" = "Power menu",
): Promise<HTMLElement> {
  const trigger = screen.getByRole("button", { name });
  trigger.focus();
  await user.keyboard("{ArrowDown}");
  return trigger;
}

describe("SPA authentication UI", () => {
  test("uses the host Chinese locale for greeter text, dates, and power actions", async () => {
    const dateFormat = vi.spyOn(Date.prototype, "toLocaleDateString");
    const transport = new MockTransport(
      snapshot([], null, ["reboot"], "zh-CN"),
    );
    await renderTheme(transport);
    const user = userEvent.setup();

    expect(document.documentElement.lang).toBe("zh-CN");
    expect(screen.getByRole("heading", { name: "谁要登录？" })).toBeVisible();
    await user.click(screen.getByRole("button", { name: /其他用户/ }));
    expect(screen.getByLabelText("用户名")).toBeEnabled();
    expect(screen.getByLabelText("密码")).toBeDisabled();
    expect(dateFormat).toHaveBeenCalledWith(
      "zh-CN",
      expect.objectContaining({ weekday: "long" }),
    );
    await openPowerMenu(user, "电源菜单");
    await user.click(screen.getByRole("menuitem", { name: "重启" }));
    expect(screen.getByRole("button", { name: "确认重启" })).toBeVisible();
    dateFormat.mockRestore();
  });

  test("localizes locker password prompts and owned failures", async () => {
    const transport = new MockTransport(
      lockerSnapshot(
        { promptId: 9, kind: "secret", message: "Password for alice:" },
        [],
        "zh-CN",
      ),
    );
    await renderTheme(transport);

    expect(screen.getByText("Fomalhaut 锁屏")).toBeVisible();
    expect(screen.getByLabelText("密码")).toBeEnabled();
    expect(screen.queryByText("Password for alice:")).not.toBeInTheDocument();
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
    expect(screen.getByRole("alert")).toHaveTextContent("认证失败，请重试。");
    expect(screen.getByRole("button", { name: "重试" })).toBeVisible();
  });

  test("preserves non-password secret prompts from PAM", async () => {
    const transport = new MockTransport(
      lockerSnapshot(
        { promptId: 10, kind: "secret", message: "Verification code:" },
        [],
        "zh-CN",
      ),
    );
    await renderTheme(transport);

    expect(screen.getByLabelText("Verification code:")).toBeEnabled();
    expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
  });

  test("uses the same localized password label for greeter prompts", async () => {
    const transport = new MockTransport(
      snapshot(
        [],
        { promptId: 11, kind: "secret", message: "Password:" },
        [],
        "zh-CN",
      ),
    );
    await renderTheme(transport);

    expect(screen.getByLabelText("密码")).toBeEnabled();
  });

  test("renders locker mode for the fixed identity without greeter controls", async () => {
    const transport = new MockTransport(lockerSnapshot());
    await renderTheme(transport);

    expect(screen.getByRole("heading", { name: "Alice" })).toBeVisible();
    const lockerLabel = screen.getByText("Fomalhaut Lock");
    expect(lockerLabel).toBeVisible();
    expect(lockerLabel.parentElement?.firstElementChild).toBe(lockerLabel);
    expect(
      screen.queryByText("Authenticate to unlock"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Who’s signing in?" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Session" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Back/ })).toBeNull();
    expect(transport.requests.at(-1)).toMatchObject({
      method: "auth.begin",
      params: {},
    });

    act(() => {
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
        data: { promptId: 9, kind: "secret", message: "Password" },
      });
    });
    expect(screen.getByLabelText("Password")).toBeEnabled();
  });

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
      screen.getByRole("button", { name: "Back to users" }).parentElement
        ?.tagName,
    ).toBe("MAIN");
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
        data: { promptId: 9, kind: "secret", message: "Password" },
      });
    });

    expect(screen.getByLabelText("Username")).toHaveValue("carol");
    expect(screen.getByLabelText("Password")).toBeEnabled();
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
    expect(transport.requests.at(-1)).toMatchObject({
      method: "auth.respond",
      params: { promptId: 7, response: "do-not-retain" },
    });
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

    await openPowerMenu(user);
    expect(
      screen.queryByRole("menuitem", { name: "Power off" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Restart" })).toBeVisible();
    await user.click(screen.getByRole("menuitem", { name: "Suspend" }));

    expect(transport.requests.at(-1)?.method).toBe("state.get");
    await user.click(screen.getByRole("button", { name: "Confirm suspend" }));
    expect(transport.requests.at(-1)).toMatchObject({
      method: "power.request",
      params: { action: "suspend" },
    });
  });

  test("dismisses the power menu through outside click", async () => {
    const transport = new MockTransport(snapshot([], null, ["reboot"]));
    await renderTheme(transport);
    const user = userEvent.setup();
    const trigger = await openPowerMenu(user);
    expect(screen.getByRole("menuitem", { name: "Restart" })).toBeVisible();
    await user.click(
      screen.getByRole("heading", { name: "Who’s signing in?" }),
    );
    await waitFor(() =>
      expect(trigger).toHaveAttribute("aria-expanded", "false"),
    );
    await waitFor(() => {
      expect(
        screen.queryByRole("menuitem", { name: "Restart" }),
      ).not.toBeInTheDocument();
    });
  });

  test("dismisses the power menu through Escape and restores focus", async () => {
    const transport = new MockTransport(snapshot([], null, ["reboot"]));
    await renderTheme(transport);
    const user = userEvent.setup();
    const trigger = await openPowerMenu(user);

    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(trigger).toHaveAttribute("aria-expanded", "false"),
    );
    expect(trigger).toHaveFocus();
    await waitFor(() => {
      expect(
        screen.queryByRole("menuitem", { name: "Restart" }),
      ).not.toBeInTheDocument();
    });
  });

  test("previews simulated power actions without a host bridge", async () => {
    await renderTheme(new DevelopmentTransport());
    const user = userEvent.setup();

    await openPowerMenu(user);
    expect(screen.getByRole("menuitem", { name: "Power off" })).toBeVisible();
    expect(screen.getByRole("menuitem", { name: "Restart" })).toBeVisible();
    await user.click(screen.getByRole("menuitem", { name: "Suspend" }));
    await user.click(screen.getByRole("button", { name: "Confirm suspend" }));

    expect(
      screen.queryByRole("button", { name: "Confirm suspend" }),
    ).not.toBeInTheDocument();
  });

  test("offers advertised power actions in locker mode", async () => {
    const transport = new MockTransport(lockerSnapshot(null, ["suspend"]));
    await renderTheme(transport);
    const user = userEvent.setup();

    await openPowerMenu(user);
    expect(screen.getByRole("menuitem", { name: "Suspend" })).toBeVisible();
    await user.click(screen.getByRole("menuitem", { name: "Suspend" }));
    await user.click(screen.getByRole("button", { name: "Confirm suspend" }));

    expect(transport.requests.at(-1)).toMatchObject({
      method: "power.request",
      params: { action: "suspend" },
    });
  });

  test("replaces a cancelled locker prompt with an explicit fresh-auth retry", async () => {
    const transport = new MockTransport(
      lockerSnapshot({ promptId: 7, kind: "secret", message: "Password" }),
    );
    await renderTheme(transport);
    const user = userEvent.setup();
    expect(screen.getByLabelText("Password")).toBeVisible();

    act(() => {
      transport.emit({
        protocol: 1,
        sequence: 1,
        event: "state.changed",
        data: { state: "idle" },
      });
    });

    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(transport.requests.at(-1)).toMatchObject({
      method: "auth.begin",
      params: {},
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
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Authentication failed. Try again.",
    );
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(transport.requests.at(-1)).toMatchObject({
      method: "auth.begin",
      params: { username: "alice" },
    });
    expect(
      screen.queryByText("Authentication failed. Try again."),
    ).not.toBeInTheDocument();
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

    trigger.focus();
    await user.keyboard("{ArrowDown}");
    expect(document.querySelector('[data-slot="select-group"]')).not.toBeNull();
    await user.click(screen.getByRole("option", { name: "X11 · x11" }));

    expect(transport.requests.at(-1)).toMatchObject({
      method: "session.select",
      params: { sessionId: "x11" },
    });
  });
});
