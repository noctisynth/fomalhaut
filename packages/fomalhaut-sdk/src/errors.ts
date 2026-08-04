import type { ProtocolErrorBody, RequestId } from "./generated/v1/index.js";

export class FomalhautProtocolError extends Error {
  public readonly requestId: RequestId;
  public readonly body: ProtocolErrorBody;

  public constructor(requestId: RequestId, body: ProtocolErrorBody) {
    super(body.message);
    this.name = "FomalhautProtocolError";
    this.requestId = requestId;
    this.body = body;
  }
}

export class FomalhautBridgeError extends Error {
  public override readonly cause: unknown;

  public constructor(message: string, cause?: unknown) {
    super(message, { cause });
    this.name = "FomalhautBridgeError";
    this.cause = cause;
  }
}

export class FomalhautBusyError extends Error {
  public constructor() {
    super("another Fomalhaut request is still in progress");
    this.name = "FomalhautBusyError";
  }
}
