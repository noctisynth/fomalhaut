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
import { useTranslation } from "react-i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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
import type { TranslationKey } from "@/i18n";
import { cn } from "@/lib/utils";
import type { ThemeScreen } from "@/state/theme-store";
import { useThemeStore } from "@/state/theme-store-provider";

function useMessages(): (key: TranslationKey) => string {
  const { t } = useTranslation();
  return t;
}

const passwordPromptPattern = /^password\s*:?\s*$/i;
const passwordForPromptPattern =
  /^password\s+for\s+[^:\s](?:[^:\r\n]*[^:\s])?\s*:?\s*$/i;

function usePromptLabel(prompt: Prompt | null): string {
  const t = useMessages();
  if (
    !prompt ||
    (prompt.kind === "secret" &&
      (passwordPromptPattern.test(prompt.message) ||
        passwordForPromptPattern.test(prompt.message)))
  ) {
    return t("form.password");
  }
  return prompt.message;
}

export function App() {
  const phase = useThemeStore((state) => state.phase);
  const snapshot = useThemeStore((state) => state.snapshot);
  const screen = useThemeStore((state) => state.screen);
  const error = useThemeStore((state) => state.error);
  const locale = useThemeStore((state) => state.locale);
  const { i18n } = useTranslation();
  const t = useMessages();

  useEffect(() => {
    document.documentElement.lang = locale;
    void i18n.changeLanguage(locale);
  }, [i18n, locale]);

  return (
    <main className="relative min-h-screen overflow-hidden bg-background text-foreground">
      <Background />
      <Clock />

      {phase === "ready" &&
        snapshot?.mode === "greeter" &&
        screen.name !== "user-selection" && (
          <AuthenticationBackButton
            label={
              screen.name === "authentication-recovery"
                ? t("back.cancel-authentication")
                : t("back.users")
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
          "absolute inset-0",
          "bg-[radial-gradient(circle_at_top_left,rgba(242,214,162,0.12),transparent_42%)]",
        )}
      />
      <div
        className={cn(
          "absolute inset-0",
          "bg-[radial-gradient(circle_at_bottom_right,rgba(57,120,214,0.30),transparent_48%)]",
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
  const locale = useThemeStore((state) => state.locale);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  return (
    <div className="absolute top-7 left-8 z-20 text-left sm:top-10 sm:left-12">
      <time className="block text-3xl font-light tracking-tight text-starlight sm:text-4xl">
        {now.toLocaleTimeString(locale, {
          hour: "2-digit",
          minute: "2-digit",
        })}
      </time>
      <time className="mt-1 block text-sm text-muted-foreground sm:text-base">
        {now.toLocaleDateString(locale, {
          weekday: "long",
          month: "long",
          day: "numeric",
        })}
      </time>
    </div>
  );
}

function LoadingView() {
  const t = useMessages();
  return (
    <div
      className="flex flex-col items-center gap-4 text-center"
      aria-live="polite"
    >
      <LoaderCircle className="size-8 animate-spin text-primary motion-reduce:animate-none" />
      <p className="text-sm text-muted-foreground">{t("loading.connecting")}</p>
    </div>
  );
}

function UnavailableView({ message }: { message: string | null }) {
  const t = useMessages();
  return (
    <div
      className={cn(
        "w-full max-w-md rounded-2xl border border-white/10",
        "bg-black/40 p-6",
      )}
    >
      <Alert variant="destructive">
        <AlertTitle>{t("unavailable.title")}</AlertTitle>
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
  const t = useMessages();
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
        {t("selection.title")}
      </h1>
      <p className="mt-3 text-sm text-muted-foreground">
        {t("selection.description")}
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
            "border-white/10 bg-[#0a1730]/85 transition duration-200",
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
            <span className="block font-medium">
              {t("selection.other-user")}
            </span>
            <span className="mt-1 block text-xs text-muted-foreground">
              {t("selection.manual-username")}
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
        "border-white/10 bg-[#0a1730]/85 transition duration-200",
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
  const t = useMessages();

  return (
    <AuthenticationLayout>
      <div className="text-center">
        <p className="mb-3 text-xs font-medium tracking-[0.34em] text-warm-star uppercase">
          Fomalhaut
        </p>
        <h1 className="text-3xl font-medium tracking-tight">
          {t("sign-in.title")}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {t("sign-in.description")}
        </p>
      </div>
      <AuthenticationFeedback />
      <ManualAuthenticationForm username={username} prompt={prompt} />
    </AuthenticationLayout>
  );
}

function AuthenticationRecoveryView() {
  const prompt = useThemeStore((state) => state.snapshot?.prompt ?? null);
  const t = useMessages();

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
        <h1 className="text-3xl font-medium tracking-tight">
          {t("authentication.title")}
        </h1>
        <p className="mt-1 max-w-sm text-sm text-muted-foreground">
          {t("authentication.recovery")}
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
  const t = useMessages();

  if (!snapshot) {
    return null;
  }

  const status = (() => {
    switch (snapshot.lock) {
      case "acquiring":
        return t("lock.acquiring");
      case "locked":
        return null;
      case "unlocking":
        return t("lock.unlocking");
      case "released":
        return t("lock.released");
      case "failed":
        return t("lock.failed");
    }
  })();

  return (
    <AuthenticationLayout>
      <p className="text-xs font-medium tracking-[0.34em] text-warm-star uppercase">
        {t("lock.brand")}
      </p>
      <UserAvatar
        user={snapshot.identity}
        className={cn(
          "size-24 border-2 border-white/20",
          "shadow-[0_0_50px_rgba(142,197,255,0.18)]",
        )}
      />
      <div className="text-center">
        <h1 className="text-3xl font-medium tracking-tight">
          {snapshot.identity.displayName}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">
          {snapshot.identity.username}
        </p>
        {status && (
          <p className="mt-3 text-sm text-muted-foreground" role="status">
            {status}
          </p>
        )}
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
        "bg-[#0a1730]/85 sm:left-10",
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
  const t = useMessages();
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
  const promptLabel = usePromptLabel(prompt);
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
        <Label htmlFor="manual-username">{t("form.username")}</Label>
        <InputGroup
          className="h-12 border-white/15 bg-[#081426]/90"
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
                aria-label={t("form.continue")}
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
        <Label htmlFor="manual-credential">{promptLabel}</Label>
        <InputGroup
          className="h-12 border-white/15 bg-[#081426]/90"
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
              username ? t("form.waiting") : t("form.username-first")
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
                  aria-label={t("form.sign-in")}
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
          className="w-full rounded-xl border-white/15 bg-[#102a52]/90"
          type="button"
          variant="outline"
          disabled={busy}
          onClick={() => void retryAuthentication()}
        >
          {t("form.try-again")}
        </Button>
      )}
    </form>
  );
}

function PromptForm({ prompt }: { prompt: Prompt }) {
  const t = useMessages();
  const promptLabel = usePromptLabel(prompt);
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
      <Label htmlFor="prompt-response">{promptLabel}</Label>
      <InputGroup className="h-12 border-white/15 bg-[#081426]/90">
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
            aria-label={t("form.sign-in")}
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
  const t = useMessages();
  const authentication = useThemeStore(
    (state) => state.snapshot?.authentication ?? "idle",
  );
  const mode = useThemeStore((state) => state.snapshot?.mode ?? null);
  const busy = useThemeStore((state) => state.busy);
  const error = useThemeStore((state) => state.error);
  const retryAuthentication = useThemeStore(
    (state) => state.retryAuthentication,
  );

  if (
    (authentication === "failed" ||
      error ||
      (mode === "locker" && authentication === "idle")) &&
    allowRetry
  ) {
    return (
      <Button
        className="w-full rounded-xl border-white/15 bg-[#102a52]/90"
        type="button"
        variant="outline"
        disabled={busy}
        onClick={() => void retryAuthentication()}
      >
        {t("form.try-again")}
      </Button>
    );
  }

  return (
    <div
      className={cn(
        "flex h-12 w-full items-center justify-center gap-3 rounded-xl border",
        "border-white/10 bg-[#081426]/90 text-sm text-muted-foreground",
      )}
    >
      <LoaderCircle className="size-4 animate-spin text-primary motion-reduce:animate-none" />
      {t("authentication.waiting")}
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
  const t = useMessages();
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
        "absolute right-6 bottom-6 z-20",
        "sm:right-10 sm:bottom-9",
      )}
    >
      <Label className="sr-only" htmlFor="session">
        {t("session.label")}
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
            "w-56 border-white/10 bg-[#081426]/90 text-starlight",
            "*:data-[slot=select-value]:min-w-0",
            "hover:bg-[#0b1a31]/90 data-[popup-open]:bg-[#0b1a31]/90",
            "focus-visible:border-primary/50 focus-visible:ring-primary/20",
          )}
        >
          <MonitorCog
            className="size-4 text-muted-foreground"
            aria-hidden="true"
          />
          <SelectValue placeholder={t("session.choose")}>
            {selectedSession ? (
              <SessionTriggerLabel session={selectedSession} />
            ) : null}
          </SelectValue>
        </SelectTrigger>
        <SelectContent side="top" align="end" className="min-w-60">
          <SelectGroup>
            {snapshot.sessions.map((session) => (
              <SelectItem key={session.id} value={session.id}>
                <SessionItemLabel session={session} />
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>
  );
}

type SessionLabelProps = {
  session: { name: string; kind: "wayland" | "x11" };
};

function SessionTriggerLabel({ session }: SessionLabelProps) {
  return (
    <span
      data-slot="session-trigger-label"
      className="grid w-full min-w-0 flex-1 grid-cols-[minmax(0,1fr)_auto] items-center gap-1.5"
    >
      <span data-slot="session-name" className="mx-1 min-w-0 flex-1 truncate">
        {session.name}
      </span>
      <span className="sr-only">, </span>
      <SessionKindBadge kind={session.kind} variant="secondary" />
    </span>
  );
}

function SessionItemLabel({ session }: SessionLabelProps) {
  return (
    <span
      data-slot="session-item-label"
      className="grid w-full min-w-0 flex-1 grid-cols-[minmax(0,1fr)_auto] items-center gap-2"
    >
      <span className="min-w-0 flex-1 truncate">{session.name}</span>
      <span className="sr-only">, </span>
      <SessionKindBadge kind={session.kind} />
    </span>
  );
}

function SessionKindBadge({
  kind,
  variant = "outline",
}: {
  kind: SessionLabelProps["session"]["kind"];
  variant?: "outline" | "secondary";
}) {
  return (
    <Badge
      variant={variant}
      className={cn(
        "justify-self-end",
        variant === "outline" && "text-muted-foreground",
      )}
    >
      {kind}
    </Badge>
  );
}

const powerLabelKeys: Record<PowerAction, TranslationKey> = {
  poweroff: "power.poweroff",
  reboot: "power.reboot",
  suspend: "power.suspend",
};

const powerQuestionKeys: Record<PowerAction, TranslationKey> = {
  poweroff: "power.question.poweroff",
  reboot: "power.question.reboot",
  suspend: "power.question.suspend",
};

const powerConfirmKeys: Record<PowerAction, TranslationKey> = {
  poweroff: "power.confirm.poweroff",
  reboot: "power.confirm.reboot",
  suspend: "power.confirm.suspend",
};

const powerIcons: Record<PowerAction, typeof Power> = {
  poweroff: Power,
  reboot: RotateCcw,
  suspend: Moon,
};

function PowerMenu() {
  const t = useMessages();
  const actions = useThemeStore(
    (state) => state.snapshot?.capabilities.power ?? [],
  );
  const busy = useThemeStore((state) => state.busy);
  const requestPower = useThemeStore((state) => state.requestPower);
  const error = useThemeStore((state) => state.error);
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
    }
  };

  return (
    <div className="absolute bottom-6 left-6 z-20 sm:bottom-9 sm:left-10">
      <DropdownMenu>
        <DropdownMenuTrigger
          className={cn(
            buttonVariants({ variant: "outline", size: "icon" }),
            "rounded-full border-white/10 bg-[#081426]/90",
          )}
          disabled={busy}
          aria-label={t("power.menu")}
        >
          <Power aria-hidden="true" />
        </DropdownMenuTrigger>
        <DropdownMenuContent
          className="w-64 border border-white/10 bg-[#081426]/95"
          side="top"
          align="start"
        >
          <DropdownMenuGroup aria-label={t("power.actions")}>
            {actions.map((action) => {
              const Icon = powerIcons[action];
              return (
                <DropdownMenuItem
                  key={action}
                  disabled={busy}
                  variant={action === "poweroff" ? "destructive" : "default"}
                  onClick={() => setConfirmation(action)}
                >
                  <Icon aria-hidden="true" />
                  {t(powerLabelKeys[action])}
                </DropdownMenuItem>
              );
            })}
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>

      <AlertDialog
        open={confirmation !== null}
        onOpenChange={(open) => {
          if (!open && !busy) {
            setConfirmation(null);
          }
        }}
      >
        {confirmation && (
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>
                {t(powerQuestionKeys[confirmation])}
              </AlertDialogTitle>
              <AlertDialogDescription>
                {t("power.warning")}
              </AlertDialogDescription>
            </AlertDialogHeader>
            {error && (
              <p className="text-sm text-destructive" role="alert">
                {error}
              </p>
            )}
            <AlertDialogFooter>
              <AlertDialogCancel disabled={busy}>
                {t("power.cancel")}
              </AlertDialogCancel>
              <AlertDialogAction
                type="button"
                variant="destructive"
                disabled={busy}
                onClick={() => void submit()}
              >
                {busy ? (
                  <LoaderCircle className="animate-spin motion-reduce:animate-none" />
                ) : null}
                {t(powerConfirmKeys[confirmation])}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        )}
      </AlertDialog>
    </div>
  );
}
