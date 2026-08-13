import { createClient } from "@/runtime/create-client";
import { createThemeStore, type ThemeStore } from "@/state/theme-store";

export interface ThemeRuntime {
  store: ThemeStore;
  initialize(): Promise<void>;
  destroy(): void;
}

export async function createThemeRuntime(): Promise<ThemeRuntime> {
  const client = await createClient();
  const runtime = createThemeStore(client);

  return {
    store: runtime.store,
    initialize: runtime.initialize,
    destroy: () => {
      runtime.destroy();
      client.close();
    },
  };
}
