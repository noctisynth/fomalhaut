# fomalhaut-sdk

Typed, framework-independent access to the Fomalhaut greeter protocol.

```ts
import { FomalhautClient } from "fomalhaut-sdk";

const client = new FomalhautClient();
const state = await client.state.get();

client.on("auth.prompt", async (prompt) => {
  const input = document.querySelector<HTMLInputElement>("#credential");
  if (!input) return;

  const response = input.value;
  input.value = "";
  await client.auth.respond(prompt.promptId, response);
});

const firstSession = state.sessions[0];
if (firstSession) {
  await client.session.select(firstSession.id);
}
await client.auth.begin("alice");
```

Authentication responses are not queued or logged by the SDK. Clear credential inputs before
awaiting a request, and only load trusted theme code in the greeter.

Generated protocol types are also available from `fomalhaut-sdk/protocol`.
