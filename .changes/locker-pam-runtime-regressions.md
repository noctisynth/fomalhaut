---
fomalhaut-pam: "patch:fix"
fomalhaut-lock: "patch:fix"
---

Stop the PAM IPC reader after its first terminal channel failure so cancellation cannot deadlock before locker power requests.

Remove user-unit seccomp hardening that implicitly enables NoNewPrivs and prevents the configured PAM stack from executing unix_chkpwd.
