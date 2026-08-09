---
fomalhaut-pam: "minor:feat"
fomalhaut-lock: "minor:feat"
fomalhaut-gtk: "minor:feat"
fomalhaut-web: "minor:feat"
---

Implement the compositor-neutral Wayland session locker with isolated PAM reauthentication, per-monitor session-lock surfaces, trusted native fallback, and systemd readiness.

Expose shared host and controller signals required to route cross-view events and fail closed when the authentication worker becomes unavailable.
