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

export interface ThemeState {
  phase: ThemePhase;
  snapshot: StateSnapshot | null;
  selectedUsername: string | null;
  manualUsername: boolean;
  selectionTouched: boolean;
  busy: boolean;
  error: string | null;
  selectUser(username: string): void;
  selectOtherUser(): void;
  beginAuthentication(username: string): Promise<void>;
  respondToPrompt(prompt: Prompt, response: string): Promise<void>;
  cancelAuthentication(): Promise<void>;
  selectSession(sessionId: string): Promise<void>;
  clearError(): void;
}

export type ThemeStore = StoreApi<ThemeState>;

export interface ThemeStoreRuntime {
  store: ThemeStore;
  initialize(): Promise<void>;
  destroy(): void;
}

export function initialUsername(users: readonly UserSummary[]): string | null {
  return users.length === 1 ? (users[0]?.username ?? null) : null;
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

export function createThemeStore(client: FomalhautClient): ThemeStoreRuntime {
  const store = createStore<ThemeState>((set, get) => {
    const run = async (operation: () => Promise<void>): Promise<void> => {
      if (get().busy) {
        return;
      }
      set({ busy: true, error: null });
      try {
        await operation();
      } catch (error) {
        set({ error: displayError(error) });
      } finally {
        set({ busy: false });
      }
    };

    return {
      phase: "loading",
      snapshot: null,
      selectedUsername: null,
      manualUsername: false,
      selectionTouched: false,
      busy: false,
      error: null,
      selectUser: (username) => {
        set({
          selectedUsername: username,
          manualUsername: false,
          selectionTouched: true,
          error: null,
        });
      },
      selectOtherUser: () => {
        set({
          selectedUsername: null,
          manualUsername: true,
          selectionTouched: true,
          error: null,
        });
      },
      beginAuthentication: (username) => run(() => client.auth.begin(username)),
      respondToPrompt: (prompt, response) =>
        run(() => client.auth.respond(prompt.promptId, response)),
      cancelAuthentication: () => run(() => client.auth.cancel()),
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
        const current = store.getState();
        store.setState({
          phase: "ready",
          snapshot,
          selectedUsername: current.selectionTouched
            ? current.selectedUsername
            : initialUsername(snapshot.users),
          manualUsername: current.selectionTouched
            ? current.manualUsername
            : snapshot.users.length === 0,
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
