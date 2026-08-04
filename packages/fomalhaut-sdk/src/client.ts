import type { FomalhautTransport, FomalhautUnsubscribe } from "./bridge.js";
import { asEventEnvelope, WebKitTransport } from "./bridge.js";
import {
  FomalhautBridgeError,
  FomalhautBusyError,
  FomalhautProtocolError,
} from "./errors.js";
import type { FomalhautEventListener, FomalhautEventName } from "./events.js";
import type {
  EmptyResult,
  EventEnvelope,
  PromptId,
  ProtocolErrorBody,
  RequestEnvelope,
  RequestId,
  ResponseResult,
  SessionSelectParams,
  StateSnapshot,
} from "./generated/v1/index.js";

const PROTOCOL_VERSION = 1 as const;
const MAX_SAFE_REQUEST_ID = Number.MAX_SAFE_INTEGER;

type EventSubscriber = (event: EventEnvelope) => void;

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

/** High-level, framework-independent client for the protocol v1 host bridge. */
export class FomalhautClient {
  readonly #transport: FomalhautTransport;
  readonly #subscribers = new Set<EventSubscriber>();
  readonly #unsubscribeTransport: FomalhautUnsubscribe;
  #nextRequestId = 1;
  #lastSequence = 0;
  #busy = false;

  public readonly state = {
    get: (): Promise<StateSnapshot> => this.#stateGet(),
  };

  public readonly session = {
    select: (sessionId: SessionSelectParams["sessionId"]): Promise<void> =>
      this.#sessionSelect(sessionId),
  };

  public readonly auth = {
    begin: (username: string): Promise<void> => this.#authBegin(username),
    respond: (promptId: PromptId, response: string): Promise<void> =>
      this.#authRespond(promptId, response),
    cancel: (): Promise<void> => this.#authCancel(),
  };

  public constructor(transport: FomalhautTransport = new WebKitTransport()) {
    this.#transport = transport;
    this.#unsubscribeTransport = transport.subscribe((value) =>
      this.#receiveEvent(value),
    );
  }

  public on<Name extends FomalhautEventName>(
    name: Name,
    listener: FomalhautEventListener<Name>,
  ): FomalhautUnsubscribe {
    const subscriber: EventSubscriber = (envelope) => {
      if (envelope.event === name) {
        listener(envelope.data as never, envelope);
      }
    };
    this.#subscribers.add(subscriber);
    return () => this.#subscribers.delete(subscriber);
  }

  public close(): void {
    this.#unsubscribeTransport();
    this.#subscribers.clear();
  }

  async #stateGet(): Promise<StateSnapshot> {
    return (await this.#exchange({
      method: "state.get",
      params: {},
    })) as StateSnapshot;
  }

  async #sessionSelect(sessionId: string): Promise<void> {
    await this.#exchange({ method: "session.select", params: { sessionId } });
  }

  async #authBegin(username: string): Promise<void> {
    await this.#exchange({ method: "auth.begin", params: { username } });
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

  async #exchange(
    request: Omit<RequestEnvelope, "protocol" | "id">,
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
    const envelope = {
      protocol: PROTOCOL_VERSION,
      id,
      ...request,
    } as RequestEnvelope;

    this.#busy = true;
    try {
      const response = await this.#transport.request(envelope);
      if (!isRecord(response)) {
        throw new FomalhautBridgeError(
          "the host returned a non-object response",
        );
      }
      if (response.protocol !== PROTOCOL_VERSION) {
        throw new FomalhautBridgeError(
          "the host returned an unsupported protocol version",
        );
      }
      if (response.id !== id) {
        throw new FomalhautBridgeError(
          "the host returned a mismatched request ID",
        );
      }
      if (response.ok === false) {
        if (!isProtocolErrorBody(response.error)) {
          throw new FomalhautBridgeError(
            "the host returned a malformed protocol error",
          );
        }
        throw new FomalhautProtocolError(id, response.error);
      }
      if (response.ok !== true || !("result" in response)) {
        throw new FomalhautBridgeError(
          "the host returned a malformed success response",
        );
      }
      return response.result as ResponseResult;
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
    } finally {
      this.#busy = false;
    }
  }

  #receiveEvent(value: unknown): void {
    if (!isRecord(value)) {
      return;
    }
    if (
      value.protocol !== PROTOCOL_VERSION ||
      typeof value.sequence !== "number" ||
      !Number.isSafeInteger(value.sequence) ||
      value.sequence <= this.#lastSequence ||
      typeof value.event !== "string" ||
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
