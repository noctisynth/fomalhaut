import type { ErrorInfo, ReactNode } from "react";
import { Component } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { detectBrowserLocale, i18n, translate } from "@/i18n";
import { cn } from "@/lib/utils";

interface AppErrorBoundaryProps {
  children: ReactNode;
}

interface AppErrorBoundaryState {
  failed: boolean;
}

function resolvedLocale() {
  return i18n.resolvedLanguage === "zh-CN" || i18n.resolvedLanguage === "en"
    ? i18n.resolvedLanguage
    : detectBrowserLocale();
}

export class AppErrorBoundary extends Component<
  AppErrorBoundaryProps,
  AppErrorBoundaryState
> {
  public state: AppErrorBoundaryState = { failed: false };

  public static getDerivedStateFromError(): AppErrorBoundaryState {
    return { failed: true };
  }

  public componentDidCatch(_error: Error, _info: ErrorInfo): void {
    // The greeter deliberately avoids logging values from the authentication UI.
    document.documentElement.lang = resolvedLocale();
  }

  public render(): ReactNode {
    if (this.state.failed) {
      const locale = resolvedLocale();
      return (
        <main
          className={cn(
            "grid min-h-screen place-items-center",
            "bg-background p-6 text-foreground",
          )}
        >
          <div className="w-full max-w-md space-y-4 text-center">
            <h1 className="text-2xl font-medium">
              {translate(locale, "error.start-title")}
            </h1>
            <Alert variant="destructive">
              <AlertTitle>{translate(locale, "error.theme-title")}</AlertTitle>
              <AlertDescription>
                {translate(locale, "error.theme-description")}
              </AlertDescription>
            </Alert>
          </div>
        </main>
      );
    }

    return this.props.children;
  }
}
