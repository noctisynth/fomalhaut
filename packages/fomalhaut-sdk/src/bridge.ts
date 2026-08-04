import type { EventEnvelope, RequestEnvelope } from "./generated/v1/index.js";

export type FomalhautEventReceiver = (event: unknown) => void;
export type FomalhautUnsubscribe = () => void;

/** Host boundary used by the client and replaceable by tests or other webview runtimes. */
export interface FomalhautTransport {
  request(request: RequestEnvelope): Promise<unknown>;
  subscribe(receiver: FomalhautEventReceiver): FomalhautUnsubscribe;
}

interface WebKitMessageHandler {
  postMessage(request: RequestEnvelope): Promise<unknown>;
}

type FomalhautWindow = Window & {
  webkit?: {
    messageHandlers?: {
      fomalhaut?: WebKitMessageHandler;
    };
  };
};

/** Default transport for Fomalhaut's WebKitGTK host. */
export class WebKitTransport implements FomalhautTransport {
  readonly #host: FomalhautWindow;

  public constructor(host: Window = window) {
    this.#host = host as FomalhautWindow;
  }

  public request(request: RequestEnvelope): Promise<unknown> {
    const handler = this.#host.webkit?.messageHandlers?.fomalhaut;
    if (!handler) {
      return Promise.reject(
        new Error("the Fomalhaut WebKit bridge is unavailable"),
      );
    }
    return handler.postMessage(request);
  }

  public subscribe(receiver: FomalhautEventReceiver): FomalhautUnsubscribe {
    const listener = (event: Event): void => {
      if (event instanceof CustomEvent) {
        receiver(event.detail);
      }
    };
    this.#host.addEventListener("fomalhaut:event", listener);
    return () => this.#host.removeEventListener("fomalhaut:event", listener);
  }
}

/** Narrows a value already validated by the client to the generated event envelope. */
export function asEventEnvelope(value: unknown): EventEnvelope {
  return value as EventEnvelope;
}
