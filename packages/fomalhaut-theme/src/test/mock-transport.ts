import type {
  FomalhautEventReceiver,
  FomalhautTransport,
  FomalhautUnsubscribe,
  RequestEnvelope,
  StateSnapshot,
  StateSnapshotFor,
  UiLocale,
} from "fomalhaut-sdk";

export class MockTransport implements FomalhautTransport {
  readonly requests: RequestEnvelope[] = [];
  readonly #receivers = new Set<FomalhautEventReceiver>();
  readonly #snapshot: StateSnapshot;
  respondPromise: Promise<void> | null = null;
  rejectMethod: RequestEnvelope["method"] | null = null;

  public constructor(snapshot: StateSnapshot) {
    this.#snapshot = snapshot;
  }

  public async request(request: RequestEnvelope): Promise<unknown> {
    this.requests.push(request);
    if (request.method === this.rejectMethod) {
      throw new Error("mock transport rejection");
    }
    if (request.method === "auth.respond" && this.respondPromise) {
      await this.respondPromise;
    }
    return {
      protocol: 1,
      id: request.id,
      ok: true,
      result:
        request.method === "state.get" ? structuredClone(this.#snapshot) : {},
    };
  }

  public subscribe(receiver: FomalhautEventReceiver): FomalhautUnsubscribe {
    this.#receivers.add(receiver);
    return () => this.#receivers.delete(receiver);
  }

  public emit(event: unknown): void {
    for (const receiver of this.#receivers) {
      receiver(event);
    }
  }
}

export function snapshot(
  users: StateSnapshotFor<"greeter">["users"] = [],
  prompt: StateSnapshot["prompt"] = null,
  power: StateSnapshot["capabilities"]["power"] = [],
  locale: UiLocale = "en",
): StateSnapshotFor<"greeter"> {
  return {
    mode: "greeter",
    locale,
    authentication: prompt
      ? prompt.kind === "secret"
        ? "waiting_for_secret"
        : "waiting_for_visible"
      : "idle",
    login: "idle",
    prompt,
    messages: [],
    sequence: 0,
    users,
    sessions: [{ id: "wayland", name: "Wayland", kind: "wayland" }],
    selectedSessionId: "wayland",
    capabilities: { power },
  };
}

export function lockerSnapshot(
  prompt: StateSnapshot["prompt"] = null,
  power: StateSnapshot["capabilities"]["power"] = [],
  locale: UiLocale = "en",
): StateSnapshotFor<"locker"> {
  return {
    mode: "locker",
    locale,
    authentication: prompt
      ? prompt.kind === "secret"
        ? "waiting_for_secret"
        : "waiting_for_visible"
      : "idle",
    lock: "locked",
    prompt,
    messages: [],
    sequence: 0,
    identity: {
      username: "alice",
      displayName: "Alice",
      avatarUrl: null,
    },
    capabilities: { power },
  };
}
