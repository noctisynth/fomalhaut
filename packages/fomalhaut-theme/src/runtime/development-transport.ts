import type {
  AuthState,
  EventEnvelope,
  FomalhautEventReceiver,
  FomalhautTransport,
  FomalhautUnsubscribe,
  Event as ProtocolEvent,
  RequestEnvelope,
  StateSnapshotFor,
} from "fomalhaut-sdk";

const DEVELOPMENT_MARKER = "FOMALHAUT_DEVELOPMENT_TRANSPORT";

export class DevelopmentTransport implements FomalhautTransport {
  readonly #receivers = new Set<FomalhautEventReceiver>();
  #sequence = 0;
  #state: StateSnapshotFor<"greeter"> = {
    mode: "greeter",
    authentication: "idle",
    login: "idle",
    prompt: null,
    messages: [],
    sequence: 0,
    users: [
      {
        username: "stargazer",
        displayName: "Stargazer",
        avatarUrl: null,
      },
    ],
    sessions: [
      { id: "wayland", name: "Wayland", kind: "wayland" },
      { id: "x11", name: "X11", kind: "x11" },
    ],
    selectedSessionId: "wayland",
    capabilities: { power: [] },
  };

  public async request(request: RequestEnvelope): Promise<unknown> {
    switch (request.method) {
      case "state.get":
        return this.#success(request.id, structuredClone(this.#state));
      case "session.select":
        this.#state.selectedSessionId = request.params.sessionId;
        this.#emit({
          event: "session.selected",
          data: { sessionId: request.params.sessionId },
        });
        return this.#success(request.id, {});
      case "auth.begin": {
        this.#state.messages = [];
        this.#setAuthentication("authenticating");
        const username =
          "username" in request.params
            ? request.params.username
            : "the current user";
        this.#state.prompt = {
          promptId: 1,
          kind: "secret",
          message: `Password for ${username}`,
        };
        this.#setAuthentication("waiting_for_secret");
        this.#emit({ event: "auth.prompt", data: this.#state.prompt });
        return this.#success(request.id, {});
      }
      case "auth.respond":
        this.#state.prompt = null;
        if (request.params.response === "fomalhaut") {
          this.#setAuthentication("authenticated");
          this.#emit({ event: "auth.succeeded", data: {} });
        } else {
          this.#state.messages = [
            { level: "error", text: "Authentication failed" },
          ];
          this.#setAuthentication("failed");
          this.#emit({ event: "auth.message", data: this.#state.messages[0] });
          this.#emit({ event: "auth.failed", data: {} });
        }
        return this.#success(request.id, {});
      case "auth.cancel":
        this.#state.prompt = null;
        this.#setAuthentication("idle");
        this.#emit({ event: "auth.cancelled", data: {} });
        return this.#success(request.id, {});
      case "power.request":
        return {
          protocol: 1,
          id: request.id,
          ok: false,
          error: {
            code: "method_disabled",
            message: "Power actions are disabled in the development fixture",
            retryable: false,
          },
        };
    }
  }

  public subscribe(receiver: FomalhautEventReceiver): FomalhautUnsubscribe {
    this.#receivers.add(receiver);
    return () => this.#receivers.delete(receiver);
  }

  #setAuthentication(state: AuthState): void {
    this.#state.authentication = state;
    this.#emit({ event: "state.changed", data: { state } });
  }

  #emit(event: ProtocolEvent): void {
    this.#sequence += 1;
    this.#state.sequence = this.#sequence;
    const envelope = {
      protocol: 1,
      sequence: this.#sequence,
      ...event,
    } as EventEnvelope;
    for (const receiver of this.#receivers) {
      receiver(envelope);
    }
  }

  #success(id: number, result: unknown): unknown {
    void DEVELOPMENT_MARKER;
    return { protocol: 1, id, ok: true, result };
  }
}
