import { createContext, type ReactNode, useContext, useRef } from "react";
import { useStore } from "zustand";
import type { ThemeState, ThemeStore } from "@/state/theme-store";

const ThemeStoreContext = createContext<ThemeStore | null>(null);

interface ThemeStoreProviderProps {
  children: ReactNode;
  store: ThemeStore;
}

export function ThemeStoreProvider({
  children,
  store,
}: ThemeStoreProviderProps) {
  const storeRef = useRef(store);
  return (
    <ThemeStoreContext.Provider value={storeRef.current}>
      {children}
    </ThemeStoreContext.Provider>
  );
}

export function useThemeStore<T>(selector: (state: ThemeState) => T): T {
  const store = useContext(ThemeStoreContext);
  if (!store) {
    throw new Error("useThemeStore must be used inside ThemeStoreProvider");
  }
  return useStore(store, selector);
}
