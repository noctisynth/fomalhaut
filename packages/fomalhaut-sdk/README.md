# fomalhaut-sdk

Typed, framework-independent access to the Fomalhaut greeter and locker protocol.

```ts
import { createFomalhautClient } from "fomalhaut-sdk";

const client = await createFomalhautClient();
const state = await client.state.get();
document.documentElement.lang = state.locale;

if (state.capabilities.power.includes("suspend")) {
  await client.power.request("suspend");
}

client.on("auth.prompt", async (prompt) => {
  const input = document.querySelector<HTMLInputElement>("#credential");
  if (!input) return;

  const response = input.value;
  input.value = "";
  await client.auth.respond(prompt.promptId, response);
});

if (client.mode === "greeter") {
  const firstSession = state.mode === "greeter" ? state.sessions[0] : undefined;
  if (firstSession) {
    await client.session.select(firstSession.id);
  }
  await client.auth.begin("alice");
} else {
  await client.auth.begin();
}
```

Authentication responses are not queued or logged by the SDK. Clear credential inputs before
awaiting a request, and only load trusted theme code in either host.

Generated protocol types are also available from `fomalhaut-sdk/protocol`.
Every bootstrapped snapshot carries the host-resolved `locale` union (`"en" | "zh-CN"`). Themes
should treat it as authoritative after bootstrap; browser locale detection is only suitable for
loading or fatal UI shown before the first snapshot.
