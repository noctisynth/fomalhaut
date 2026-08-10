---
fomalhaut-greetd: "patch:fix"
fomalhaut: "patch:fix"
---

Treat greetd authentication errors as already-cancelled failures so the greeter clears stale prompts and can retry without sending a redundant CancelSession request.
