import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app";
import { AppErrorBoundary } from "@/app-error-boundary";
import { createThemeRuntime } from "@/runtime/theme-runtime";
import { ThemeStoreProvider } from "@/state/theme-store-provider";
import "@/index.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("the application root element is missing");
}

const runtime = await createThemeRuntime();

createRoot(rootElement).render(
  <StrictMode>
    <AppErrorBoundary>
      <ThemeStoreProvider store={runtime.store}>
        <App />
      </ThemeStoreProvider>
    </AppErrorBoundary>
  </StrictMode>,
);

void runtime.initialize();
