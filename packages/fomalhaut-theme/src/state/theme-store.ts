import {
  type AuthState,
  FomalhautBridgeError,
  FomalhautBusyError,
  type FomalhautClient,
  FomalhautProtocolError,
  type Prompt,
  type StateSnapshot,
  type UserSummary,
} from "fomalhaut-sdk";
import { createStore, type StoreApi } from "zustand/vanilla";

export type ThemePhase = "loading" | "ready" | "failed";

export type ThemeScreen =
  | { name: "user-selection" }
  | { name: "known-user"; user: UserSummary }
  | { name: "other-user"; username: string | null }
  | { name: "authentication-recovery" };

export interface ThemeState {
  phase: ThemePhase;
  screen: ThemeScreen;
  snapshot: StateSnapshot | null;
  busy: boolean;
  error: string | null;
  chooseKnownUser(user: UserSummary): Promise<boolean>;
  chooseOtherUser(): void;
  submitManualUsername(username: string): Promise<boolean>;
  retryAuthentication(): Promise<boolean>;
  respondToPrompt(prompt: Prompt, response: string): Promise<boolean>;
  cancelAndReturn(): Promise<boolean>;
  selectSession(sessionId: string): Promise<boolean>;
  clearError(): void;
}

export type ThemeStore = StoreApi<ThemeState>;

export interface ThemeStoreRuntime {
  store: ThemeStore;
  initialize(): Promise<void>;
  destroy(): void;
}

function displayError(error: unknown): string {
  if (error instanceof FomalhautProtocolError) {
    return error.message;
  }
  if (error instanceof FomalhautBusyError) {
    return "Another greeter request is still in progress.";
  }
  if (error instanceof FomalhautBridgeError) {
    return "The Fomalhaut host is unavailable.";
  }
  return "The greeter could not complete the request.";
}

function authenticationIsActive(authentication: AuthState): boolean {
  return [
    "authenticating",
    "waiting_for_prompt",
    "authenticated",
    "starting_session",
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

export function createThemeStore(client: FomalhautClient): ThemeStoreRuntime {
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
        set({ error: displayError(error) });
        return false;
      } finally {
        set({ busy: false });
      }
    };

    const begin = async (username: string): Promise<boolean> => {
      clearAuthenticationDisplay(store);
      return run(() => client.auth.begin(username));
    };

    return {
      phase: "loading",
      screen: { name: "user-selection" },
      snapshot: null,
      busy: false,
      error: null,
      chooseKnownUser: async (user) => {
        set({ screen: { name: "known-user", user } });
        return begin(user.username);
      },
      chooseOtherUser: () => {
        clearAuthenticationDisplay(store);
        set({ screen: { name: "other-user", username: null } });
      },
      submitManualUsername: async (username) => {
        set({ screen: { name: "other-user", username } });
        return begin(username);
      },
      retryAuthentication: () => {
        const { screen } = get();
        if (screen.name === "known-user") {
          return begin(screen.user.username);
        }
        if (screen.name === "other-user" && screen.username) {
          return begin(screen.username);
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
        set({ screen: { name: "user-selection" } });
        return true;
      },
      selectSession: (sessionId) => run(() => client.session.select(sessionId)),
      clearError: () => set({ error: null }),
    };
  });

  const updateAuthentication = (authentication: AuthState): void => {
    store.setState((state) => ({
      snapshot: state.snapshot
        ? { ...state.snapshot, authentication }
        : state.snapshot,
    }));
  };

  const unsubscribe = [
    client.on("state.changed", ({ state }) => updateAuthentication(state)),
    client.on("auth.prompt", (prompt) => {
      store.setState((state) => ({
        snapshot: state.snapshot ? { ...state.snapshot, prompt } : null,
      }));
    }),
    client.on("auth.message", (message) => {
      store.setState((state) => ({
        snapshot: state.snapshot
          ? {
              ...state.snapshot,
              messages: [...state.snapshot.messages, message],
            }
          : null,
      }));
    }),
    client.on("auth.failed", () => clearPrompt(store)),
    client.on("auth.cancelled", () => clearPrompt(store)),
    client.on("auth.succeeded", () => clearPrompt(store)),
    client.on("session.selected", ({ sessionId }) => {
      store.setState((state) => ({
        snapshot: state.snapshot
          ? { ...state.snapshot, selectedSessionId: sessionId }
          : null,
      }));
    }),
    client.on("session.started", () => updateAuthentication("started")),
  ];

  return {
    store,
    initialize: async () => {
      try {
        const snapshot = await client.state.get();
        store.setState({
          phase: "ready",
          snapshot,
          screen: authenticationIsActive(snapshot.authentication)
            ? { name: "authentication-recovery" }
            : { name: "user-selection" },
          error: null,
        });
      } catch (error) {
        store.setState({ phase: "failed", error: displayError(error) });
      }
    },
    destroy: () => {
      for (const stop of unsubscribe) {
        stop();
      }
    },
  };
}

function clearPrompt(store: ThemeStore): void {
  store.setState((state) => ({
    snapshot: state.snapshot ? { ...state.snapshot, prompt: null } : null,
  }));
}
