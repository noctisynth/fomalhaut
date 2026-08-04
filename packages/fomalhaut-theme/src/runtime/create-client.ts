import {
  FomalhautClient,
  type FomalhautTransport,
  WebKitTransport,
} from "fomalhaut-sdk";

type BridgeWindow = Window & {
  webkit?: {
    messageHandlers?: {
      fomalhaut?: unknown;
    };
  };
};

function hasHostBridge(host: Window): boolean {
  return Boolean((host as BridgeWindow).webkit?.messageHandlers?.fomalhaut);
}

export async function createClient(): Promise<FomalhautClient> {
  let transport: FomalhautTransport = new WebKitTransport();

  if (import.meta.env.DEV && !hasHostBridge(window)) {
    const { DevelopmentTransport } = await import(
      "@/runtime/development-transport"
    );
    transport = new DevelopmentTransport();
  }

  return new FomalhautClient(transport);
}
