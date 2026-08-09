import type {
  IdentitySummary,
  PowerAction,
  Prompt,
  UserSummary,
} from "fomalhaut-sdk";
import {
  ArrowLeft,
  ArrowRight,
  LoaderCircle,
  LockKeyhole,
  MonitorCog,
  Moon,
  Power,
  RotateCcw,
  UserRound,
  UserRoundPlus,
} from "lucide-react";
import type { FormEvent, ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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

      {phase === "ready" &&
        snapshot?.mode === "greeter" &&
        screen.name !== "user-selection" && (
          <AuthenticationBackButton
            label={
              screen.name === "authentication-recovery"
                ? "Cancel authentication"
                : "Back to users"
            }
          />
        )}

      <div className="relative z-10 grid min-h-screen place-items-center px-6 py-28">
        {phase === "loading" && <LoadingView />}
        {phase === "failed" && <UnavailableView message={error} />}
        {phase === "ready" && snapshot && <Screen screen={screen} />}
      </div>

      {phase === "ready" && snapshot?.mode === "greeter" && <SessionControl />}
      {phase === "ready" && snapshot && <PowerMenu />}
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
        <AlertTitle>Fomalhaut unavailable</AlertTitle>
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
    case "locker":
      return <LockerView />;
  }
}

function UserSelectionView() {
  const users = useThemeStore((state) =>
    state.snapshot?.mode === "greeter" ? state.snapshot.users : [],
  );
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
    <AuthenticationLayout>
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
    <AuthenticationLayout>
      <div className="text-center">
        <p className="mb-3 text-xs font-medium tracking-[0.34em] text-warm-star uppercase">
          Fomalhaut
        </p>
        <h1 className="text-3xl font-medium tracking-tight">Sign in</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Enter a local or directory account.
        </p>
      </div>
      <AuthenticationFeedback />
      <ManualAuthenticationForm username={username} prompt={prompt} />
    </AuthenticationLayout>
  );
}

function AuthenticationRecoveryView() {
  const prompt = useThemeStore((state) => state.snapshot?.prompt ?? null);

  return (
    <AuthenticationLayout>
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

function LockerView() {
  const snapshot = useThemeStore((state) =>
    state.snapshot?.mode === "locker" ? state.snapshot : null,
  );

  if (!snapshot) {
    return null;
  }

  const status = (() => {
    switch (snapshot.lock) {
      case "acquiring":
        return "Securing this session…";
      case "locked":
        return "Authenticate to unlock";
      case "unlocking":
        return "Unlocking…";
      case "released":
        return "Session unlocked";
      case "failed":
        return "The native session lock is unavailable";
    }
  })();

  return (
    <AuthenticationLayout>
      <UserAvatar
        user={snapshot.identity}
        className={cn(
          "size-24 border-2 border-white/20",
          "shadow-[0_0_50px_rgba(142,197,255,0.18)]",
        )}
      />
      <div className="text-center">
        <p className="mb-3 text-xs font-medium tracking-[0.34em] text-warm-star uppercase">
          Fomalhaut Lock
        </p>
        <h1 className="text-3xl font-medium tracking-tight">
          {snapshot.identity.displayName}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {snapshot.identity.username}
        </p>
        <p className="mt-3 text-sm text-muted-foreground" role="status">
          {status}
        </p>
      </div>
      <AuthenticationFeedback />
      {snapshot.lock === "locked" && snapshot.prompt ? (
        <PromptForm prompt={snapshot.prompt} />
      ) : snapshot.lock === "locked" ? (
        <AuthenticationWaiting />
      ) : null}
    </AuthenticationLayout>
  );
}

function AuthenticationLayout({ children }: { children: ReactNode }) {
  return (
    <section
      className={cn(
        "flex w-full max-w-md flex-col items-center gap-6",
        "animate-in fade-in duration-200 motion-reduce:animate-none",
      )}
    >
      {children}
    </section>
  );
}

function AuthenticationBackButton({ label }: { label: string }) {
  const busy = useThemeStore((state) => state.busy);
  const cancelAndReturn = useThemeStore((state) => state.cancelAndReturn);

  return (
    <Button
      className={cn(
        "fixed top-28 left-6 z-20 gap-2 rounded-full border-white/10",
        "bg-black/10 backdrop-blur-md sm:left-10",
      )}
      type="button"
      variant="outline"
      disabled={busy}
      onClick={() => void cancelAndReturn()}
    >
      <ArrowLeft className="size-4" aria-hidden="true" />
      {label}
    </Button>
  );
}

function ManualAuthenticationForm({
  username,
  prompt,
}: {
  username: string | null;
  prompt: Prompt | null;
}) {
  const busy = useThemeStore((state) => state.busy);
  const authentication = useThemeStore(
    (state) => state.snapshot?.authentication ?? "idle",
  );
  const error = useThemeStore((state) => state.error);
  const submitManualUsername = useThemeStore(
    (state) => state.submitManualUsername,
  );
  const respondToPrompt = useThemeStore((state) => state.respondToPrompt);
  const retryAuthentication = useThemeStore(
    (state) => state.retryAuthentication,
  );
  const usernameInput = useRef<HTMLInputElement>(null);
  const responseInput = useRef<HTMLInputElement>(null);

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (!username) {
      const enteredUsername = usernameInput.current?.value.trim() ?? "";
      if (enteredUsername.length > 0) {
        void submitManualUsername(enteredUsername);
      }
      return;
    }
    if (prompt) {
      let response = responseInput.current?.value ?? "";
      if (responseInput.current) {
        responseInput.current.value = "";
      }
      const pending = respondToPrompt(prompt, response);
      response = "";
      void pending;
    }
  };

  const canRetry = Boolean(
    username && !prompt && (authentication === "failed" || error),
  );

  return (
    <form className="w-full space-y-4" onSubmit={submit}>
      <div className="space-y-2">
        <Label htmlFor="manual-username">Username</Label>
        <InputGroup
          className="h-12 border-white/15 bg-black/20 backdrop-blur-xl"
          data-disabled={busy || Boolean(username)}
        >
          <InputGroupAddon>
            <UserRound aria-hidden="true" />
          </InputGroupAddon>
          <InputGroupInput
            key={username ?? "username-entry"}
            id="manual-username"
            ref={usernameInput}
            defaultValue={username ?? ""}
            name="username"
            autoComplete="username"
            autoFocus={!username}
            disabled={busy || Boolean(username)}
            required={!username}
          />
          {!username && (
            <InputGroupAddon align="inline-end">
              <InputGroupButton
                type="submit"
                variant="default"
                size="icon-sm"
                disabled={busy}
                aria-label="Continue"
              >
                {busy ? (
                  <LoaderCircle className="animate-spin motion-reduce:animate-none" />
                ) : (
                  <ArrowRight />
                )}
              </InputGroupButton>
            </InputGroupAddon>
          )}
        </InputGroup>
      </div>
      <div className={cn("space-y-2", !prompt && "opacity-60")}>
        <Label htmlFor="manual-credential">
          {prompt?.message ?? "Password"}
        </Label>
        <InputGroup
          className="h-12 border-white/15 bg-black/20 backdrop-blur-xl"
          data-disabled={busy || !prompt}
        >
          <InputGroupAddon>
            <LockKeyhole aria-hidden="true" />
          </InputGroupAddon>
          <InputGroupInput
            id="manual-credential"
            ref={responseInput}
            type={prompt?.kind === "visible" ? "text" : "password"}
            placeholder={
              username ? "Waiting for authentication…" : "Enter username first"
            }
            autoComplete={
              prompt?.kind === "secret" ? "current-password" : "off"
            }
            autoFocus={Boolean(prompt)}
            disabled={busy || !prompt}
            required={Boolean(prompt)}
          />
          {username && (
            <InputGroupAddon align="inline-end">
              {prompt ? (
                <InputGroupButton
                  type="submit"
                  variant="default"
                  size="icon-sm"
                  disabled={busy}
                  aria-label="Sign in"
                >
                  {busy ? (
                    <LoaderCircle className="animate-spin motion-reduce:animate-none" />
                  ) : (
                    <ArrowRight />
                  )}
                </InputGroupButton>
              ) : (
                !canRetry && (
                  <LoaderCircle className="animate-spin motion-reduce:animate-none" />
                )
              )}
            </InputGroupAddon>
          )}
        </InputGroup>
      </div>
      {canRetry && (
        <Button
          className="w-full rounded-xl border-white/15 bg-white/10 backdrop-blur-xl"
          type="button"
          variant="outline"
          disabled={busy}
          onClick={() => void retryAuthentication()}
        >
          Try again
        </Button>
      )}
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
      <InputGroup className="h-12 border-white/15 bg-black/20 backdrop-blur-xl">
        <InputGroupAddon>
          <LockKeyhole aria-hidden="true" />
        </InputGroupAddon>
        <InputGroupInput
          id="prompt-response"
          ref={responseInput}
          type={prompt.kind === "secret" ? "password" : "text"}
          autoComplete={prompt.kind === "secret" ? "current-password" : "off"}
          autoFocus
          disabled={busy}
          required
        />
        <InputGroupAddon align="inline-end">
          <InputGroupButton
            type="submit"
            variant="default"
            size="icon-sm"
            disabled={busy}
            aria-label="Sign in"
          >
            {busy ? (
              <LoaderCircle className="animate-spin motion-reduce:animate-none" />
            ) : (
              <ArrowRight />
            )}
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>
    </form>
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
  user: UserSummary | IdentitySummary;
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
  if (snapshot?.mode !== "greeter") {
    return null;
  }
  const selectedSession = snapshot.sessions.find(
    (session) => session.id === snapshot.selectedSessionId,
  );

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
      <Select
        value={snapshot.selectedSessionId}
        disabled={busy}
        onValueChange={(value) => {
          if (value) {
            void selectSession(value);
          }
        }}
      >
        <SelectTrigger
          id="session"
          className={cn(
            "w-52 border-white/10 bg-black/20 text-starlight backdrop-blur-xl",
            "focus-visible:border-primary/60 focus-visible:ring-primary/30",
          )}
        >
          <SelectValue placeholder="Choose session">
            {selectedSession
              ? `${selectedSession.name} · ${selectedSession.kind}`
              : null}
          </SelectValue>
        </SelectTrigger>
        <SelectContent side="top" align="end">
          <SelectGroup>
            {snapshot.sessions.map((session) => (
              <SelectItem key={session.id} value={session.id}>
                {session.name} · {session.kind}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>
  );
}

const powerLabels: Record<PowerAction, string> = {
  poweroff: "Power off",
  reboot: "Restart",
  suspend: "Suspend",
};

const powerIcons: Record<PowerAction, typeof Power> = {
  poweroff: Power,
  reboot: RotateCcw,
  suspend: Moon,
};

function PowerMenu() {
  const actions = useThemeStore(
    (state) => state.snapshot?.capabilities.power ?? [],
  );
  const busy = useThemeStore((state) => state.busy);
  const requestPower = useThemeStore((state) => state.requestPower);
  const error = useThemeStore((state) => state.error);
  const [open, setOpen] = useState(false);
  const [confirmation, setConfirmation] = useState<PowerAction | null>(null);

  if (actions.length === 0) {
    return null;
  }

  const submit = async (): Promise<void> => {
    if (!confirmation) {
      return;
    }
    if (await requestPower(confirmation)) {
      setConfirmation(null);
      setOpen(false);
    }
  };

  return (
    <div className="absolute bottom-6 left-6 z-20 sm:bottom-9 sm:left-10">
      {open && (
        <div
          className={cn(
            "absolute bottom-12 left-0 w-64 rounded-2xl border border-white/10",
            "bg-black/50 p-2 shadow-2xl backdrop-blur-2xl",
          )}
        >
          {confirmation ? (
            <div className="space-y-3 p-2">
              <p className="text-sm font-medium">
                {powerLabels[confirmation]} this device?
              </p>
              <p className="text-xs text-muted-foreground">
                Active work in other sessions may be interrupted.
              </p>
              {error && (
                <p className="text-xs text-destructive" role="alert">
                  {error}
                </p>
              )}
              <div className="flex justify-end gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => setConfirmation(null)}
                >
                  Cancel
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="destructive"
                  disabled={busy}
                  onClick={() => void submit()}
                >
                  {busy ? (
                    <LoaderCircle className="animate-spin motion-reduce:animate-none" />
                  ) : null}
                  Confirm {powerLabels[confirmation].toLocaleLowerCase()}
                </Button>
              </div>
            </div>
          ) : (
            <fieldset className="grid gap-1" aria-label="Power actions">
              {actions.map((action) => {
                const Icon = powerIcons[action];
                return (
                  <Button
                    key={action}
                    className="justify-start"
                    type="button"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => setConfirmation(action)}
                  >
                    <Icon aria-hidden="true" />
                    {powerLabels[action]}
                  </Button>
                );
              })}
            </fieldset>
          )}
        </div>
      )}
      <Button
        className="rounded-full border-white/10 bg-black/20 backdrop-blur-xl"
        type="button"
        size="icon"
        variant="outline"
        disabled={busy}
        aria-label="Power menu"
        aria-expanded={open}
        onClick={() => {
          setOpen((value) => !value);
          setConfirmation(null);
        }}
      >
        <Power aria-hidden="true" />
      </Button>
    </div>
  );
}
