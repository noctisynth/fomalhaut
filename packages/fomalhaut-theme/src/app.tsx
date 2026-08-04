import type { Prompt, UserSummary } from "fomalhaut-sdk";
import type { FormEvent } from "react";
import { useRef } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import { useThemeStore } from "@/state/theme-store-provider";

export function App() {
  const phase = useThemeStore((state) => state.phase);
  const snapshot = useThemeStore((state) => state.snapshot);
  const error = useThemeStore((state) => state.error);

  return (
    <main
      className={cn(
        "relative grid min-h-screen place-items-center overflow-hidden",
        "bg-background px-5 py-10 text-foreground",
      )}
    >
      <div
        className={cn(
          "pointer-events-none absolute -inset-[35%] blur-2xl",
          "[background:radial-gradient(circle_at_50%_45%,oklch(0.72_0.12_220/22%),transparent_28%),radial-gradient(circle_at_30%_70%,oklch(0.58_0.15_280/16%),transparent_24%)]",
        )}
        aria-hidden="true"
      />
      <section className="relative z-10 w-full max-w-xl" aria-label="Sign in">
        <header className="mb-7 text-center">
          <p
            className={cn(
              "mb-2 text-xs font-medium tracking-[0.32em]",
              "text-muted-foreground uppercase",
            )}
          >
            Northern watcher
          </p>
          <h1 className="text-4xl font-semibold tracking-tight sm:text-5xl">
            Fomalhaut
          </h1>
        </header>

        {phase === "loading" && <LoadingCard />}
        {phase === "failed" && <UnavailableCard message={error} />}
        {phase === "ready" && snapshot && <LoginCard />}
      </section>
    </main>
  );
}

function LoadingCard() {
  return (
    <Card
      className={cn(
        "border-white/10 bg-card/90",
        "shadow-2xl backdrop-blur-xl",
      )}
    >
      <CardHeader>
        <CardTitle>Connecting to the greeter</CardTitle>
        <CardDescription>Reading trusted users and sessions…</CardDescription>
      </CardHeader>
    </Card>
  );
}

function UnavailableCard({ message }: { message: string | null }) {
  return (
    <Card
      className={cn(
        "border-white/10 bg-card/90",
        "shadow-2xl backdrop-blur-xl",
      )}
    >
      <CardHeader>
        <CardTitle>Greeter unavailable</CardTitle>
        <CardDescription>
          The theme could not connect to the trusted Fomalhaut host.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Alert variant="destructive">
          <AlertTitle>Connection failed</AlertTitle>
          <AlertDescription>{message}</AlertDescription>
        </Alert>
      </CardContent>
    </Card>
  );
}

function LoginCard() {
  const snapshot = useThemeStore((state) => state.snapshot);
  const busy = useThemeStore((state) => state.busy);
  const error = useThemeStore((state) => state.error);
  const cancelAuthentication = useThemeStore(
    (state) => state.cancelAuthentication,
  );

  if (!snapshot) {
    return null;
  }

  const isAuthenticating = [
    "authenticating",
    "waiting_for_prompt",
    "cancelling",
  ].includes(snapshot.authentication);

  return (
    <Card
      className={cn(
        "border-white/10 bg-card/90",
        "shadow-2xl backdrop-blur-xl",
      )}
    >
      <CardHeader>
        <CardTitle>Welcome back</CardTitle>
        <CardDescription>
          Choose an account and authenticate through greetd.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        {error && (
          <Alert variant="destructive" aria-live="polite">
            <AlertTitle>Request failed</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <AuthenticationMessages />

        {snapshot.prompt ? (
          <PromptForm prompt={snapshot.prompt} />
        ) : (
          <IdentityForm />
        )}

        <Separator />
        <SessionField />
      </CardContent>
      {isAuthenticating && (
        <CardFooter>
          <Button
            className="w-full"
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() => void cancelAuthentication()}
          >
            Cancel authentication
          </Button>
        </CardFooter>
      )}
    </Card>
  );
}

function AuthenticationMessages() {
  const messages = useThemeStore((state) => state.snapshot?.messages ?? []);
  const latest = messages.at(-1);
  if (!latest) {
    return null;
  }

  return (
    <Alert variant={latest.level === "error" ? "destructive" : "default"}>
      <AlertTitle>
        {latest.level === "error" ? "Authentication" : "Information"}
      </AlertTitle>
      <AlertDescription>{latest.text}</AlertDescription>
    </Alert>
  );
}

function IdentityForm() {
  const snapshot = useThemeStore((state) => state.snapshot);
  const selectedUsername = useThemeStore((state) => state.selectedUsername);
  const manualUsername = useThemeStore((state) => state.manualUsername);
  const busy = useThemeStore((state) => state.busy);
  const selectUser = useThemeStore((state) => state.selectUser);
  const selectOtherUser = useThemeStore((state) => state.selectOtherUser);
  const beginAuthentication = useThemeStore(
    (state) => state.beginAuthentication,
  );
  const usernameInput = useRef<HTMLInputElement>(null);

  if (!snapshot) {
    return null;
  }

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const username = manualUsername
      ? (usernameInput.current?.value.trim() ?? "")
      : (selectedUsername ?? "");
    if (username.length > 0) {
      void beginAuthentication(username);
    }
  };

  return (
    <form className="space-y-5" onSubmit={submit}>
      {snapshot.users.length > 0 && (
        <fieldset className="space-y-3" disabled={busy}>
          <legend className="text-sm font-medium">Account</legend>
          <div className="grid gap-2 sm:grid-cols-2">
            {snapshot.users.map((user) => (
              <UserButton
                key={user.username}
                user={user}
                selected={!manualUsername && selectedUsername === user.username}
                onSelect={() => selectUser(user.username)}
              />
            ))}
          </div>
        </fieldset>
      )}

      <Button
        className="w-full"
        type="button"
        variant={manualUsername ? "secondary" : "ghost"}
        disabled={busy}
        onClick={selectOtherUser}
      >
        Other user
      </Button>

      {manualUsername && (
        <div className="space-y-2">
          <Label htmlFor="username">Username</Label>
          <Input
            id="username"
            ref={usernameInput}
            name="username"
            autoComplete="username"
            autoFocus
            disabled={busy}
            required
          />
        </div>
      )}

      <Button
        className="w-full"
        type="submit"
        disabled={busy || (!manualUsername && !selectedUsername)}
      >
        {busy ? "Please wait…" : "Continue"}
      </Button>
    </form>
  );
}

function UserButton({
  user,
  selected,
  onSelect,
}: {
  user: UserSummary;
  selected: boolean;
  onSelect(): void;
}) {
  const fallback = user.displayName.trim().charAt(0).toLocaleUpperCase() || "?";
  return (
    <Button
      className={cn(
        "h-auto justify-start gap-3 px-3 py-3 text-left",
        selected && "border-primary bg-accent text-accent-foreground",
      )}
      type="button"
      variant="outline"
      aria-pressed={selected}
      onClick={onSelect}
    >
      <Avatar size="lg">
        {user.avatarUrl && (
          <AvatarImage
            src={user.avatarUrl}
            alt=""
            referrerPolicy="no-referrer"
          />
        )}
        <AvatarFallback>{fallback}</AvatarFallback>
      </Avatar>
      <span className="min-w-0">
        <span className="block truncate font-medium">{user.displayName}</span>
        <span className="block truncate text-xs text-muted-foreground">
          {user.username}
        </span>
      </span>
    </Button>
  );
}

function PromptForm({ prompt }: { prompt: Prompt }) {
  const busy = useThemeStore((state) => state.busy);
  const respondToPrompt = useThemeStore((state) => state.respondToPrompt);
  const responseInput = useRef<HTMLInputElement>(null);

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    let response = responseInput.current?.value ?? "";
    if (responseInput.current) {
      responseInput.current.value = "";
    }
    const pending = respondToPrompt(prompt, response);
    response = "";
    void pending;
  };

  return (
    <form className="space-y-4" onSubmit={submit}>
      <div className="space-y-2">
        <Label htmlFor="prompt-response">{prompt.message}</Label>
        <Input
          id="prompt-response"
          ref={responseInput}
          type={prompt.kind === "secret" ? "password" : "text"}
          autoComplete={prompt.kind === "secret" ? "current-password" : "off"}
          autoFocus
          disabled={busy}
          required
        />
      </div>
      <Button className="w-full" type="submit" disabled={busy}>
        {busy ? "Authenticating…" : "Sign in"}
      </Button>
    </form>
  );
}

function SessionField() {
  const snapshot = useThemeStore((state) => state.snapshot);
  const busy = useThemeStore((state) => state.busy);
  const selectSession = useThemeStore((state) => state.selectSession);
  if (!snapshot) {
    return null;
  }

  return (
    <div className="space-y-2">
      <Label htmlFor="session">Session</Label>
      <select
        id="session"
        className={cn(
          "flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs outline-none",
          "focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
        )}
        value={snapshot.selectedSessionId ?? ""}
        disabled={busy}
        onChange={(event) => void selectSession(event.currentTarget.value)}
      >
        {!snapshot.selectedSessionId && (
          <option value="">Choose a session</option>
        )}
        {snapshot.sessions.map((session) => (
          <option key={session.id} value={session.id}>
            {session.name} · {session.kind}
          </option>
        ))}
      </select>
    </div>
  );
}
