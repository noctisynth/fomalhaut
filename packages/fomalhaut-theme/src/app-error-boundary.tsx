import type { ErrorInfo, ReactNode } from "react";
import { Component } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

interface AppErrorBoundaryProps {
  children: ReactNode;
}

interface AppErrorBoundaryState {
  failed: boolean;
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
  }

  public render(): ReactNode {
    if (this.state.failed) {
      return (
        <main
          className={cn(
            "grid min-h-screen place-items-center",
            "bg-background p-6 text-foreground",
          )}
        >
          <Card className="w-full max-w-md">
            <CardHeader>
              <CardTitle>Fomalhaut could not start</CardTitle>
            </CardHeader>
            <CardContent>
              <Alert variant="destructive">
                <AlertTitle>Theme error</AlertTitle>
                <AlertDescription>
                  Restart the greeter or ask an administrator to inspect the
                  host.
                </AlertDescription>
              </Alert>
            </CardContent>
          </Card>
        </main>
      );
    }

    return this.props.children;
  }
}
