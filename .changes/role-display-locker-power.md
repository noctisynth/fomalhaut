---
fomalhaut-config: "minor:feat"
fomalhaut-logind: "minor:feat"
fomalhaut-web: "minor:feat"
fomalhaut: "patch:feat"
fomalhaut-lock: "minor:feat"
---

Add shared or per-role display scaling and use the shared non-interactive logind backend for both greeter and locker power actions.

Keep the session lock held while locker power requests cancel any active reauthentication transaction.
