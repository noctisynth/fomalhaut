import type { FomalhautTransport, FomalhautUnsubscribe } from "./bridge.js";
import { asEventEnvelope, WebKitTransport } from "./bridge.js";
import {
  FomalhautBridgeError,
  FomalhautBusyError,
  FomalhautProtocolError,
} from "./errors.js";
import type {
  FomalhautEventData,
  FomalhautEventEnvelope,
  FomalhautEventListener,
  FomalhautEventName,
} from "./events.js";
import type {
  EmptyResult,
  Event,
  EventEnvelope,
  FrontendRequest,
  GreeterAuthBeginParams,
  LockerAuthBeginParams,
  PowerAction,
  PromptId,
  ProtocolErrorBody,
  RequestEnvelope,
  RequestId,
  ResponseResult,
  RuntimeMode,
  SessionSelectParams,
  StateSnapshot,
} from "./generated/v1/index.js";

const PROTOCOL_VERSION = 1 as const;
const MAX_SAFE_REQUEST_ID = Number.MAX_SAFE_INTEGER;
const EVENT_NAMES = new Set<Event["event"]>([
  "state.changed",
  "auth.prompt",
  "auth.message",
  "auth.succeeded",
  "auth.failed",
  "auth.cancelled",
  "session.selected",
  "session.started",
  "lock.acquired",
  "lock.failed",
  "lock.released",
]);

type EventSubscriber = (event: EventEnvelope) => void;

/** Generated snapshot branch belonging to one runtime mode. */
export type StateSnapshotFor<M extends RuntimeMode> = Extract<
  StateSnapshot,
  { mode: M }
>;

/** Generated `auth.begin` parameters belonging to one runtime mode. */
export type AuthBeginParamsFor<M extends RuntimeMode> = M extends "greeter"
  ? GreeterAuthBeginParams
  : LockerAuthBeginParams;

/** Role-specific public argument tuple for `auth.begin`. */
export type AuthBeginArgs<M extends RuntimeMode> = M extends "greeter"
  ? [username: GreeterAuthBeginParams["username"]]
  : [];

export interface FomalhautAuthFacade<M extends RuntimeMode> {
  begin(...args: AuthBeginArgs<M>): Promise<void>;
  respond(promptId: PromptId, response: string): Promise<void>;
  cancel(): Promise<void>;
}

export interface FomalhautSessionFacade {
  select(sessionId: SessionSelectParams["sessionId"]): Promise<void>;
}

/** Locker clients have no callable session facade. */
export type SessionFacadeFor<M extends RuntimeMode> = M extends "greeter"
  ? FomalhautSessionFacade
  : undefined;

export type AnyFomalhautClient =
  | FomalhautClient<"greeter">
  | FomalhautClient<"locker">;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isProtocolErrorBody(value: unknown): value is ProtocolErrorBody {
  return (
    isRecord(value) &&
    typeof value.code === "string" &&
    typeof value.message === "string" &&
    typeof value.retryable === "boolean"
  );
}

function requireStateSnapshot(value: unknown): StateSnapshot;
function requireStateSnapshot<M extends RuntimeMode>(
  value: unknown,
  expectedMode: M,
): StateSnapshotFor<M>;
function requireStateSnapshot(
  value: unknown,
  expectedMode?: RuntimeMode,
): StateSnapshot {
  if (
    !isRecord(value) ||
    (value.mode !== "greeter" && value.mode !== "locker") ||
    (value.locale !== "en" && value.locale !== "zh-CN") ||
    typeof value.sequence !== "number" ||
    !Number.isSafeInteger(value.sequence) ||
    value.sequence < 0
  ) {
    throw new FomalhautBridgeError(
      "the host returned a malformed state snapshot",
    );
  }
  if (expectedMode !== undefined && value.mode !== expectedMode) {
    throw new FomalhautBridgeError(
      "the host returned a state snapshot for another runtime mode",
    );
  }
  return value as StateSnapshot;
}

function isEventName(value: string): value is Event["event"] {
  return EVENT_NAMES.has(value as Event["event"]);
}

function eventBelongsToMode(mode: RuntimeMode, event: Event["event"]): boolean {
  if (event.startsWith("session.")) {
    return mode === "greeter";
  }
  if (event.startsWith("lock.")) {
    return mode === "locker";
  }
  return true;
}

function decodeResponse(value: unknown, id: RequestId): ResponseResult {
  if (!isRecord(value)) {
    throw new FomalhautBridgeError("the host returned a non-object response");
  }
  if (value.protocol !== PROTOCOL_VERSION) {
    throw new FomalhautBridgeError(
      "the host returned an unsupported protocol version",
    );
  }
  if (value.id !== id) {
    throw new FomalhautBridgeError("the host returned a mismatched request ID");
  }
  if (value.ok === false) {
    if (!isProtocolErrorBody(value.error)) {
      throw new FomalhautBridgeError(
        "the host returned a malformed protocol error",
      );
    }
    throw new FomalhautProtocolError(id, value.error);
  }
  if (value.ok !== true || !("result" in value)) {
    throw new FomalhautBridgeError(
      "the host returned a malformed success response",
    );
  }
  return value.result as ResponseResult;
}

async function exchangeTransport(
  transport: FomalhautTransport,
  id: RequestId,
  request: FrontendRequest,
): Promise<ResponseResult | EmptyResult> {
  const envelope = {
    protocol: PROTOCOL_VERSION,
    id,
    ...request,
  } as RequestEnvelope;
  try {
    return decodeResponse(await transport.request(envelope), id);
  } catch (error) {
    if (
      error instanceof FomalhautBridgeError ||
      error instanceof FomalhautProtocolError
    ) {
      throw error;
    }
    throw new FomalhautBridgeError(
      "the Fomalhaut bridge request failed",
      error,
    );
  }
}

/** Bootstrapped, framework-independent client for one protocol v1 host mode. */
export class FomalhautClient<M extends RuntimeMode> {
  readonly #transport: FomalhautTransport;
  readonly #subscribers = new Set<EventSubscriber>();
  readonly #unsubscribeTransport: FomalhautUnsubscribe;
  #nextRequestId = 2;
  #lastSequence: number;
  #busy = false;

  public readonly mode: M;
  public readonly state: { get: () => Promise<StateSnapshotFor<M>> };
  public readonly session: SessionFacadeFor<M>;
  public readonly auth: FomalhautAuthFacade<M>;
  public readonly power: { request: (action: PowerAction) => Promise<void> };

  private constructor(
    mode: M,
    sequence: number,
    transport: FomalhautTransport,
    unsubscribeTransport: FomalhautUnsubscribe,
    queuedEvents: readonly unknown[],
    activateReceiver: (receiver: (value: unknown) => void) => void,
  ) {
    this.mode = mode;
    this.#lastSequence = sequence;
    this.#transport = transport;
    this.#unsubscribeTransport = unsubscribeTransport;
    this.state = { get: () => this.#stateGet() };
    this.auth = {
      begin: (...args: AuthBeginArgs<M>) => this.#authBegin(args),
      respond: (promptId, response) => this.#authRespond(promptId, response),
      cancel: () => this.#authCancel(),
    };
    this.session = (
      mode === "greeter"
        ? { select: (sessionId: string) => this.#sessionSelect(sessionId) }
        : undefined
    ) as SessionFacadeFor<M>;
    this.power = { request: (action) => this.#powerRequest(action) };
    for (const event of queuedEvents) {
      this.receiveEvent(event);
    }
    activateReceiver((value) => this.receiveEvent(value));
  }

  /** Subscribes first, then bootstraps mode and sequence from `state.get`. */
  public static async connect(
    transport: FomalhautTransport = new WebKitTransport(),
  ): Promise<AnyFomalhautClient> {
    const queuedEvents: unknown[] = [];
    let receive: (value: unknown) => void = (value) => queuedEvents.push(value);
    const unsubscribe = transport.subscribe((value) => receive(value));

    try {
      const result = await exchangeTransport(transport, 1, {
        method: "state.get",
        params: {},
      });
      const snapshot = requireStateSnapshot(result);
      const activateReceiver = (receiver: (value: unknown) => void): void => {
        receive = receiver;
      };
      if (snapshot.mode === "greeter") {
        return new FomalhautClient(
          "greeter",
          snapshot.sequence,
          transport,
          unsubscribe,
          queuedEvents,
          activateReceiver,
        );
      }
      return new FomalhautClient(
        "locker",
        snapshot.sequence,
        transport,
        unsubscribe,
        queuedEvents,
        activateReceiver,
      );
    } catch (error) {
      unsubscribe();
      throw error;
    }
  }

  public on<Name extends FomalhautEventName<M>>(
    name: Name,
    listener: FomalhautEventListener<M, Name>,
  ): FomalhautUnsubscribe {
    const subscriber: EventSubscriber = (envelope) => {
      if (envelope.event === name) {
        listener(
          envelope.data as FomalhautEventData<M, Name>,
          envelope as FomalhautEventEnvelope<M, Name>,
        );
      }
    };
    this.#subscribers.add(subscriber);
    return () => this.#subscribers.delete(subscriber);
  }

  public close(): void {
    this.#unsubscribeTransport();
    this.#subscribers.clear();
  }

  async #stateGet(): Promise<StateSnapshotFor<M>> {
    const result = await this.#exchange({
      method: "state.get",
      params: {},
    });
    const snapshot = requireStateSnapshot(result, this.mode);
    if (snapshot.sequence > this.#lastSequence) {
      this.#lastSequence = snapshot.sequence;
    }
    return snapshot;
  }

  async #sessionSelect(sessionId: string): Promise<void> {
    await this.#exchange({ method: "session.select", params: { sessionId } });
  }

  async #authBegin(args: AuthBeginArgs<M>): Promise<void> {
    if (this.mode === "greeter") {
      const [username] = args as AuthBeginArgs<"greeter">;
      await this.#exchange({ method: "auth.begin", params: { username } });
      return;
    }
    await this.#exchange({ method: "auth.begin", params: {} });
  }

  async #authRespond(promptId: PromptId, response: string): Promise<void> {
    await this.#exchange({
      method: "auth.respond",
      params: { promptId, response },
    });
  }

  async #authCancel(): Promise<void> {
    await this.#exchange({ method: "auth.cancel", params: {} });
  }

  async #powerRequest(action: PowerAction): Promise<void> {
    await this.#exchange({ method: "power.request", params: { action } });
  }

  async #exchange(
    request: FrontendRequest,
  ): Promise<ResponseResult | EmptyResult> {
    if (this.#busy) {
      throw new FomalhautBusyError();
    }
    if (this.#nextRequestId > MAX_SAFE_REQUEST_ID) {
      throw new FomalhautBridgeError(
        "the frontend request counter is exhausted",
      );
    }

    const id: RequestId = this.#nextRequestId;
    this.#nextRequestId += 1;
    this.#busy = true;
    try {
      return await exchangeTransport(this.#transport, id, request);
    } finally {
      this.#busy = false;
    }
  }

  private receiveEvent(value: unknown): void {
    if (!isRecord(value)) {
      return;
    }
    if (
      value.protocol !== PROTOCOL_VERSION ||
      typeof value.sequence !== "number" ||
      !Number.isSafeInteger(value.sequence) ||
      value.sequence <= this.#lastSequence ||
      typeof value.event !== "string" ||
      !isEventName(value.event) ||
      !eventBelongsToMode(this.mode, value.event) ||
      !("data" in value)
    ) {
      return;
    }
    this.#lastSequence = value.sequence;
    const envelope = asEventEnvelope(value);
    for (const subscriber of this.#subscribers) {
      subscriber(envelope);
    }
  }
}

/** Creates a bootstrapped client whose `mode` discriminates all role-specific APIs. */
export async function createFomalhautClient(
  transport: FomalhautTransport = new WebKitTransport(),
): Promise<AnyFomalhautClient> {
  return FomalhautClient.connect(transport);
}
