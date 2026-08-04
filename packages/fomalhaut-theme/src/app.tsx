import type { Prompt, UserSummary } from "fomalhaut-sdk";
import {
  ArrowLeft,
  ArrowRight,
  LoaderCircle,
  MonitorCog,
  UserRound,
  UserRoundPlus,
} from "lucide-react";
import type { FormEvent, ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import type { ThemeScreen } from "@/state/theme-store";
import { useThemeStore } from "@/state/theme-store-provider";

export function App() {
  const phase = useThemeStore((state) => state.phase);
  const snapshot = useThemeStore((state) => state.snapshot);
  const screen = useThemeStore((state) => state.screen);
  const error = useThemeStore((state) => state.error);

  return (
    <main
      className={cn(
        "relative min-h-screen overflow-hidden bg-background text-foreground",
        "selection:bg-primary/30 selection:text-foreground",
      )}
    >
      <Background />
      <Clock />

      <div className="relative z-10 grid min-h-screen place-items-center px-6 py-28">
        {phase === "loading" && <LoadingView />}
        {phase === "failed" && <UnavailableView message={error} />}
        {phase === "ready" && snapshot && <Screen screen={screen} />}
      </div>

      {phase === "ready" && snapshot && <SessionControl />}
    </main>
  );
}

function Background() {
  return (
    <div className="pointer-events-none absolute inset-0" aria-hidden="true">
      <div
        className={cn(
          "absolute -top-[35%] -left-[20%] size-[75vw] rounded-full blur-3xl",
          "bg-[radial-gradient(circle,rgba(242,214,162,0.12),transparent_62%)]",
        )}
      />
      <div
        className={cn(
          "absolute -right-[20%] -bottom-[45%] size-[95vw] rounded-full blur-3xl",
          "bg-[radial-gradient(circle,rgba(57,120,214,0.30),transparent_60%)]",
        )}
      />
      <div
        className={cn(
          "absolute inset-0",
          "bg-[linear-gradient(135deg,rgba(5,8,18,0.2),rgba(16,42,82,0.22))]",
        )}
      />
    </div>
  );
}

function Clock() {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  return (
    <div className="absolute top-7 left-8 z-20 text-left sm:top-10 sm:left-12">
      <time className="block text-3xl font-light tracking-tight text-starlight sm:text-4xl">
        {now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
      </time>
      <time className="mt-1 block text-sm text-muted-foreground sm:text-base">
        {now.toLocaleDateString([], {
          weekday: "long",
          month: "long",
          day: "numeric",
        })}
      </time>
    </div>
  );
}

function LoadingView() {
  return (
    <div
      className="flex flex-col items-center gap-4 text-center"
      aria-live="polite"
    >
      <LoaderCircle className="size-8 animate-spin text-primary motion-reduce:animate-none" />
      <p className="text-sm text-muted-foreground">Connecting to Fomalhaut…</p>
    </div>
  );
}

function UnavailableView({ message }: { message: string | null }) {
  return (
    <div
      className={cn(
        "w-full max-w-md rounded-2xl border border-white/10",
        "bg-black/20 p-6 backdrop-blur-xl",
      )}
    >
      <Alert variant="destructive">
        <AlertTitle>Greeter unavailable</AlertTitle>
        <AlertDescription>{message}</AlertDescription>
      </Alert>
    </div>
  );
}

function Screen({ screen }: { screen: ThemeScreen }) {
  switch (screen.name) {
    case "user-selection":
      return <UserSelectionView />;
    case "known-user":
      return <KnownUserView user={screen.user} />;
    case "other-user":
      return <OtherUserView username={screen.username} />;
    case "authentication-recovery":
      return <AuthenticationRecoveryView />;
  }
}

function UserSelectionView() {
  const users = useThemeStore((state) => state.snapshot?.users ?? []);
  const busy = useThemeStore((state) => state.busy);
  const chooseKnownUser = useThemeStore((state) => state.chooseKnownUser);
  const chooseOtherUser = useThemeStore((state) => state.chooseOtherUser);

  return (
    <section
      className={cn(
        "w-full max-w-4xl text-center",
        "animate-in fade-in zoom-in-95 duration-200 motion-reduce:animate-none",
      )}
      aria-labelledby="selection-title"
    >
      <p className="mb-3 text-xs font-medium tracking-[0.34em] text-warm-star uppercase">
        Fomalhaut
      </p>
      <h1
        id="selection-title"
        className="text-3xl font-light tracking-tight sm:text-5xl"
      >
        Who’s signing in?
      </h1>
      <p className="mt-3 text-sm text-muted-foreground">
        Choose an account to continue on this device.
      </p>

      <div
        className={cn(
          "mx-auto mt-10 flex max-w-4xl flex-wrap justify-center gap-3",
          "[&>*]:w-full sm:[&>*]:w-64",
        )}
        data-testid="account-list"
      >
        {users.map((user) => (
          <AccountTile
            key={user.username}
            user={user}
            disabled={busy}
            onSelect={() => void chooseKnownUser(user)}
          />
        ))}
        <button
          className={cn(
            "group flex min-h-24 items-center gap-4 rounded-2xl border p-4 text-left",
            "border-white/10 bg-white/5 backdrop-blur-md transition duration-200",
            "hover:-translate-y-0.5 hover:border-primary/50 hover:bg-white/10",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/70",
            "disabled:pointer-events-none disabled:opacity-50 motion-reduce:transform-none",
          )}
          type="button"
          disabled={busy}
          onClick={chooseOtherUser}
        >
          <span
            className={cn(
              "grid size-14 shrink-0 place-items-center rounded-full border",
              "border-white/10 bg-white/5 text-muted-foreground transition",
              "group-hover:text-primary",
            )}
          >
            <UserRoundPlus className="size-6" aria-hidden="true" />
          </span>
          <span>
            <span className="block font-medium">Other user</span>
            <span className="mt-1 block text-xs text-muted-foreground">
              Enter a username manually
            </span>
          </span>
        </button>
      </div>
    </section>
  );
}

function AccountTile({
  user,
  disabled,
  onSelect,
}: {
  user: UserSummary;
  disabled: boolean;
  onSelect(): void;
}) {
  return (
    <button
      className={cn(
        "group flex min-h-24 items-center gap-4 rounded-2xl border p-4 text-left",
        "border-white/10 bg-white/5 backdrop-blur-md transition duration-200",
        "hover:-translate-y-0.5 hover:border-primary/50 hover:bg-white/10",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/70",
        "disabled:pointer-events-none disabled:opacity-50 motion-reduce:transform-none",
      )}
      type="button"
      disabled={disabled}
      onClick={onSelect}
    >
      <UserAvatar user={user} className="size-14" />
      <span className="min-w-0">
        <span className="block truncate font-medium">{user.displayName}</span>
        <span className="mt-1 block truncate text-xs text-muted-foreground">
          {user.username}
        </span>
      </span>
    </button>
  );
}

function KnownUserView({ user }: { user: UserSummary }) {
  const prompt = useThemeStore((state) => state.snapshot?.prompt ?? null);

  return (
    <AuthenticationLayout onBackLabel="Back to users">
      <UserAvatar
        user={user}
        className={cn(
          "size-24 border-2 border-white/20",
          "shadow-[0_0_50px_rgba(142,197,255,0.18)]",
        )}
      />
      <div className="text-center">
        <h1 className="text-3xl font-medium tracking-tight">
          {user.displayName}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">{user.username}</p>
      </div>
      <AuthenticationFeedback />
      {prompt ? <PromptForm prompt={prompt} /> : <AuthenticationWaiting />}
    </AuthenticationLayout>
  );
}

function OtherUserView({ username }: { username: string | null }) {
  const prompt = useThemeStore((state) => state.snapshot?.prompt ?? null);

  return (
    <AuthenticationLayout onBackLabel="Back to users">
      <div
        className={cn(
          "grid size-24 place-items-center rounded-full border-2 border-white/15",
          "bg-white/5 text-muted-foreground",
          "shadow-[0_0_50px_rgba(142,197,255,0.12)]",
        )}
      >
        <UserRound className="size-10" aria-hidden="true" />
      </div>
      <div className="text-center">
        <h1 className="text-3xl font-medium tracking-tight">Other user</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {username ?? "Sign in with a local or directory account"}
        </p>
      </div>
      <AuthenticationFeedback />
      {username ? (
        prompt ? (
          <PromptForm prompt={prompt} />
        ) : (
          <AuthenticationWaiting />
        )
      ) : (
        <ManualIdentityForm />
      )}
    </AuthenticationLayout>
  );
}

function AuthenticationRecoveryView() {
  const prompt = useThemeStore((state) => state.snapshot?.prompt ?? null);

  return (
    <AuthenticationLayout onBackLabel="Cancel authentication">
      <div
        className={cn(
          "grid size-24 place-items-center rounded-full border-2 border-primary/30",
          "bg-primary/10 text-primary",
          "shadow-[0_0_50px_rgba(142,197,255,0.16)]",
        )}
      >
        <UserRound className="size-10" aria-hidden="true" />
      </div>
      <div className="text-center">
        <h1 className="text-3xl font-medium tracking-tight">Authentication</h1>
        <p className="mt-1 max-w-sm text-sm text-muted-foreground">
          The host has an active sign-in without recoverable identity details.
        </p>
      </div>
      <AuthenticationFeedback />
      {prompt ? (
        <PromptForm prompt={prompt} />
      ) : (
        <AuthenticationWaiting allowRetry={false} />
      )}
    </AuthenticationLayout>
  );
}

function AuthenticationLayout({
  children,
  onBackLabel,
}: {
  children: ReactNode;
  onBackLabel: string;
}) {
  const busy = useThemeStore((state) => state.busy);
  const cancelAndReturn = useThemeStore((state) => state.cancelAndReturn);

  return (
    <section
      className={cn(
        "flex w-full max-w-md flex-col items-center gap-6",
        "animate-in fade-in zoom-in-95 duration-200 motion-reduce:animate-none",
      )}
    >
      <Button
        className={cn(
          "fixed top-28 left-6 gap-2 rounded-full border-white/10",
          "bg-black/10 backdrop-blur-md sm:left-10",
        )}
        type="button"
        variant="outline"
        disabled={busy}
        onClick={() => void cancelAndReturn()}
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        {onBackLabel}
      </Button>
      {children}
    </section>
  );
}

function ManualIdentityForm() {
  const busy = useThemeStore((state) => state.busy);
  const submitManualUsername = useThemeStore(
    (state) => state.submitManualUsername,
  );
  const usernameInput = useRef<HTMLInputElement>(null);

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const username = usernameInput.current?.value.trim() ?? "";
    if (username.length > 0) {
      void submitManualUsername(username);
    }
  };

  return (
    <form className="w-full space-y-4" onSubmit={submit}>
      <div className="space-y-2">
        <Label htmlFor="username">Username</Label>
        <div className="relative">
          <Input
            id="username"
            ref={usernameInput}
            className="h-12 border-white/15 bg-black/20 pr-12 text-base backdrop-blur-xl"
            name="username"
            autoComplete="username"
            autoFocus
            disabled={busy}
            required
          />
          <SubmitArrow disabled={busy} label="Continue" />
        </div>
      </div>
      <div className="space-y-2 opacity-55">
        <Label htmlFor="pending-credential">Authentication prompt</Label>
        <Input
          id="pending-credential"
          className="h-12 border-white/10 bg-black/10"
          type="password"
          placeholder="Available after username"
          disabled
        />
      </div>
    </form>
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
    <form className="w-full space-y-2" onSubmit={submit}>
      <Label htmlFor="prompt-response">{prompt.message}</Label>
      <div className="relative">
        <Input
          id="prompt-response"
          ref={responseInput}
          className={cn(
            "h-12 border-white/15 bg-black/20 pr-12 text-base backdrop-blur-xl",
            "focus-visible:border-primary/70 focus-visible:ring-primary/30",
          )}
          type={prompt.kind === "secret" ? "password" : "text"}
          autoComplete={prompt.kind === "secret" ? "current-password" : "off"}
          autoFocus
          disabled={busy}
          required
        />
        <SubmitArrow disabled={busy} label="Sign in" />
      </div>
    </form>
  );
}

function SubmitArrow({
  disabled,
  label,
}: {
  disabled: boolean;
  label: string;
}) {
  return (
    <button
      className={cn(
        "absolute top-1.5 right-1.5 grid size-9 place-items-center rounded-md",
        "bg-primary text-primary-foreground transition hover:bg-primary/90",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/60",
        "disabled:pointer-events-none disabled:opacity-50",
      )}
      type="submit"
      disabled={disabled}
      aria-label={label}
    >
      {disabled ? (
        <LoaderCircle className="size-4 animate-spin motion-reduce:animate-none" />
      ) : (
        <ArrowRight className="size-4" />
      )}
    </button>
  );
}

function AuthenticationWaiting({
  allowRetry = true,
}: {
  allowRetry?: boolean;
}) {
  const authentication = useThemeStore(
    (state) => state.snapshot?.authentication ?? "idle",
  );
  const busy = useThemeStore((state) => state.busy);
  const error = useThemeStore((state) => state.error);
  const retryAuthentication = useThemeStore(
    (state) => state.retryAuthentication,
  );

  if ((authentication === "failed" || error) && allowRetry) {
    return (
      <Button
        className="w-full rounded-xl border-white/15 bg-white/10 backdrop-blur-xl"
        type="button"
        variant="outline"
        disabled={busy}
        onClick={() => void retryAuthentication()}
      >
        Try again
      </Button>
    );
  }

  return (
    <div
      className={cn(
        "flex h-12 w-full items-center justify-center gap-3 rounded-xl border",
        "border-white/10 bg-black/10 text-sm text-muted-foreground backdrop-blur-xl",
      )}
    >
      <LoaderCircle className="size-4 animate-spin text-primary motion-reduce:animate-none" />
      Waiting for the authentication service…
    </div>
  );
}

function AuthenticationFeedback() {
  const error = useThemeStore((state) => state.error);
  const latestMessage = useThemeStore((state) =>
    state.snapshot?.messages.at(-1),
  );
  const message = error ?? latestMessage?.text;
  const destructive = Boolean(error || latestMessage?.level === "error");

  if (!message) {
    return null;
  }

  return (
    <p
      className={cn(
        "w-full text-center text-sm",
        destructive ? "text-destructive" : "text-muted-foreground",
      )}
      role={destructive ? "alert" : "status"}
      aria-live="polite"
    >
      {message}
    </p>
  );
}

function UserAvatar({
  user,
  className,
}: {
  user: UserSummary;
  className?: string;
}) {
  const fallback = user.displayName.trim().charAt(0).toLocaleUpperCase() || "?";
  return (
    <Avatar className={cn("size-14 bg-white/5", className)}>
      {user.avatarUrl && (
        <AvatarImage src={user.avatarUrl} alt="" referrerPolicy="no-referrer" />
      )}
      <AvatarFallback className="bg-white/5 text-lg text-starlight">
        {fallback}
      </AvatarFallback>
    </Avatar>
  );
}

function SessionControl() {
  const snapshot = useThemeStore((state) => state.snapshot);
  const busy = useThemeStore((state) => state.busy);
  const selectSession = useThemeStore((state) => state.selectSession);
  if (!snapshot) {
    return null;
  }

  return (
    <div
      className={cn(
        "absolute right-6 bottom-6 z-20 flex items-center gap-3",
        "sm:right-10 sm:bottom-9",
      )}
    >
      <MonitorCog className="size-4 text-muted-foreground" aria-hidden="true" />
      <Label className="sr-only" htmlFor="session">
        Session
      </Label>
      <select
        id="session"
        className={cn(
          "h-9 max-w-48 rounded-full border border-white/10 bg-black/20 px-4 text-sm",
          "text-starlight backdrop-blur-xl outline-none transition",
          "focus-visible:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30",
          "disabled:cursor-not-allowed disabled:opacity-50",
        )}
        value={snapshot.selectedSessionId ?? ""}
        disabled={busy}
        onChange={(event) => void selectSession(event.currentTarget.value)}
      >
        {!snapshot.selectedSessionId && (
          <option value="">Choose session</option>
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
