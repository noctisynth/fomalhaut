import {
  type AnyFomalhautClient,
  type AuthState,
  FomalhautBridgeError,
  FomalhautBusyError,
  type FomalhautClient,
  FomalhautProtocolError,
  type FomalhautUnsubscribe,
  type PowerAction,
  type Prompt,
  type RuntimeMode,
  type StateSnapshot,
  type UiLocale,
  type UserSummary,
} from "fomalhaut-sdk";
import { createStore, type StoreApi } from "zustand/vanilla";
import { detectBrowserLocale, translate } from "@/i18n";

const MAX_AUTHENTICATION_MESSAGES = 16;

export type ThemePhase = "loading" | "ready" | "failed";

export type ThemeScreen =
  | { name: "user-selection" }
  | { name: "known-user"; user: UserSummary }
  | { name: "other-user"; username: string | null }
  | { name: "authentication-recovery" }
  | { name: "locker" };

export interface ThemeState {
  phase: ThemePhase;
  screen: ThemeScreen;
  snapshot: StateSnapshot | null;
  locale: UiLocale;
  busy: boolean;
  error: string | null;
  chooseKnownUser(user: UserSummary): Promise<boolean>;
  chooseOtherUser(): void;
  submitManualUsername(username: string): Promise<boolean>;
  retryAuthentication(): Promise<boolean>;
  respondToPrompt(prompt: Prompt, response: string): Promise<boolean>;
  cancelAndReturn(): Promise<boolean>;
  selectSession(sessionId: string): Promise<boolean>;
  requestPower(action: PowerAction): Promise<boolean>;
  clearError(): void;
}

export type ThemeStore = StoreApi<ThemeState>;

export interface ThemeStoreRuntime {
  store: ThemeStore;
  initialize(): Promise<void>;
  destroy(): void;
}

function displayError(error: unknown, locale: UiLocale): string {
  if (error instanceof FomalhautProtocolError) {
    return error.message;
  }
  if (error instanceof FomalhautBusyError) {
    return translate(locale, "error.busy");
  }
  if (error instanceof FomalhautBridgeError) {
    return translate(locale, "error.host-unavailable");
  }
  return translate(locale, "error.request-failed");
}

function authenticationIsActive(authentication: AuthState): boolean {
  return [
    "authenticating",
    "waiting_for_secret",
    "waiting_for_visible",
    "authenticated",
    "cancelling",
  ].includes(authentication);
}

function clearAuthenticationDisplay(store: ThemeStore): void {
  store.setState((state) => ({
    snapshot: state.snapshot
      ? { ...state.snapshot, prompt: null, messages: [] }
      : null,
    error: null,
  }));
}

export function createThemeStore(
  client: AnyFomalhautClient,
): ThemeStoreRuntime {
  const store = createStore<ThemeState>((set, get) => {
    const run = async (operation: () => Promise<void>): Promise<boolean> => {
      if (get().busy) {
        return false;
      }
      set({ busy: true, error: null });
      try {
        await operation();
        return true;
      } catch (error) {
        set({ error: displayError(error, get().locale) });
        return false;
      } finally {
        set({ busy: false });
      }
    };

    const beginGreeter = async (username: string): Promise<boolean> => {
      if (client.mode !== "greeter") {
        return false;
      }
      clearAuthenticationDisplay(store);
      return run(() => client.auth.begin(username));
    };

    return {
      phase: "loading",
      screen: { name: "user-selection" },
      snapshot: null,
      locale: detectBrowserLocale(),
      busy: false,
      error: null,
      chooseKnownUser: async (user) => {
        if (client.mode !== "greeter") {
          return false;
        }
        set({ screen: { name: "known-user", user } });
        return beginGreeter(user.username);
      },
      chooseOtherUser: () => {
        if (client.mode !== "greeter") {
          return;
        }
        clearAuthenticationDisplay(store);
        set({ screen: { name: "other-user", username: null } });
      },
      submitManualUsername: async (username) => {
        if (client.mode !== "greeter") {
          return false;
        }
        set({ screen: { name: "other-user", username } });
        return beginGreeter(username);
      },
      retryAuthentication: () => {
        clearAuthenticationDisplay(store);
        if (client.mode === "locker") {
          return run(() => client.auth.begin());
        }
        const { screen } = get();
        if (screen.name === "known-user") {
          return beginGreeter(screen.user.username);
        }
        if (screen.name === "other-user" && screen.username) {
          return beginGreeter(screen.username);
        }
        return Promise.resolve(false);
      },
      respondToPrompt: (prompt, response) =>
        run(() => client.auth.respond(prompt.promptId, response)),
      cancelAndReturn: async () => {
        const snapshot = get().snapshot;
        if (
          snapshot &&
          authenticationIsActive(snapshot.authentication) &&
          !(await run(() => client.auth.cancel()))
        ) {
          return false;
        }
        clearAuthenticationDisplay(store);
        set({
          screen:
            client.mode === "locker"
              ? { name: "locker" }
              : { name: "user-selection" },
        });
        return true;
      },
      selectSession: (sessionId) => {
        if (client.mode !== "greeter") {
          return Promise.resolve(false);
        }
        return run(() => client.session.select(sessionId));
      },
      requestPower: (action) => run(() => client.power.request(action)),
      clearError: () => set({ error: null }),
    };
  });

  const updateAuthentication = (
    authentication: AuthState,
    sequence: number,
  ): void => {
    store.setState((state) => ({
      snapshot: state.snapshot
        ? { ...state.snapshot, authentication, sequence }
        : state.snapshot,
    }));
  };

  const subscribeCommonEvents = <M extends RuntimeMode>(
    client: FomalhautClient<M>,
  ): FomalhautUnsubscribe[] => [
    client.on("state.changed", ({ state }, envelope) =>
      updateAuthentication(state, envelope.sequence),
    ),
    client.on("auth.prompt", (prompt, envelope) => {
      store.setState((state) => ({
        snapshot: state.snapshot
          ? { ...state.snapshot, prompt, sequence: envelope.sequence }
          : null,
      }));
    }),
    client.on("auth.message", (message, envelope) => {
      store.setState((state) => ({
        snapshot: state.snapshot
          ? {
              ...state.snapshot,
              messages: [...state.snapshot.messages, message],
              sequence: envelope.sequence,
            }
          : null,
      }));
    }),
    client.on("auth.failed", (_, envelope) =>
      recordAuthenticationFailure(store, envelope.sequence),
    ),
    client.on("auth.cancelled", (_, envelope) =>
      clearPrompt(store, envelope.sequence),
    ),
    client.on("auth.succeeded", (_, envelope) =>
      clearPrompt(store, envelope.sequence),
    ),
  ];

  const unsubscribe: FomalhautUnsubscribe[] = [];

  if (client.mode === "greeter") {
    unsubscribe.push(
      ...subscribeCommonEvents(client),
      client.on("session.selected", ({ sessionId }, envelope) => {
        store.setState((state) => ({
          snapshot:
            state.snapshot?.mode === "greeter"
              ? {
                  ...state.snapshot,
                  selectedSessionId: sessionId,
                  sequence: envelope.sequence,
                }
              : state.snapshot,
        }));
      }),
      client.on("session.started", (_, envelope) => {
        store.setState((state) => ({
          snapshot:
            state.snapshot?.mode === "greeter"
              ? {
                  ...state.snapshot,
                  login: "started",
                  sequence: envelope.sequence,
                }
              : state.snapshot,
        }));
      }),
    );
  } else {
    const updateLock = (
      lock: Extract<StateSnapshot, { mode: "locker" }>["lock"],
      sequence: number,
    ): void => {
      store.setState((state) => ({
        snapshot:
          state.snapshot?.mode === "locker"
            ? { ...state.snapshot, lock, sequence }
            : state.snapshot,
      }));
    };
    unsubscribe.push(
      ...subscribeCommonEvents(client),
      client.on("lock.acquired", (_, envelope) =>
        updateLock("locked", envelope.sequence),
      ),
      client.on("lock.failed", (_, envelope) =>
        updateLock("failed", envelope.sequence),
      ),
      client.on("lock.released", (_, envelope) =>
        updateLock("released", envelope.sequence),
      ),
    );
  }

  return {
    store,
    initialize: async () => {
      try {
        const snapshot = await client.state.get();
        if (snapshot.mode === "locker") {
          store.setState({
            phase: "ready",
            snapshot,
            locale: snapshot.locale,
            screen: { name: "locker" },
            error: null,
          });
          if (
            snapshot.lock === "locked" &&
            (snapshot.authentication === "idle" ||
              snapshot.authentication === "failed")
          ) {
            await store.getState().retryAuthentication();
          }
          return;
        }

        const singleUser =
          snapshot.users.length === 1 ? snapshot.users[0] : undefined;
        store.setState({
          phase: "ready",
          snapshot,
          locale: snapshot.locale,
          screen: authenticationIsActive(snapshot.authentication)
            ? { name: "authentication-recovery" }
            : singleUser
              ? { name: "known-user", user: singleUser }
              : { name: "user-selection" },
          error: null,
        });
        if (!authenticationIsActive(snapshot.authentication) && singleUser) {
          await store.getState().chooseKnownUser(singleUser);
        }
      } catch (error) {
        store.setState({
          phase: "failed",
          error: displayError(error, store.getState().locale),
        });
      }
    },
    destroy: () => {
      for (const stop of unsubscribe) {
        stop();
      }
    },
  };
}

function clearPrompt(store: ThemeStore, sequence: number): void {
  store.setState((state) => ({
    snapshot: state.snapshot
      ? { ...state.snapshot, prompt: null, sequence }
      : null,
  }));
}

function recordAuthenticationFailure(
  store: ThemeStore,
  sequence: number,
): void {
  store.setState((state) => {
    if (!state.snapshot) {
      return { snapshot: null };
    }

    const messages = state.snapshot.messages;
    const latestMessage = messages.at(-1);
    return {
      snapshot: {
        ...state.snapshot,
        prompt: null,
        messages:
          latestMessage?.level === "error"
            ? messages
            : [
                ...messages.slice(-(MAX_AUTHENTICATION_MESSAGES - 1)),
                {
                  level: "error",
                  text: translate(state.locale, "authentication.failure"),
                },
              ],
        sequence,
      },
    };
  });
}
