# Changelog

## v0.1.0-alpha.1

### Bug Fixes

- [`27c5ee8`](https://github.com/noctisynth/fomalhaut/commit/27c5ee8e57a99546534a39069edc09585497ec45): Normalize standard password prompts to the active theme locale in both greeter and locker while preserving OTP, PIN, visible, and custom PAM prompts.
- [`2c274c9`](https://github.com/noctisynth/fomalhaut/commit/2c274c901a5ec73d79c45755a77f2a1ce882d99b): Restart locker authentication with a fresh PAM transaction after system resume and prevent themes from submitting prompts cancelled before suspend.

### New Features

- [`20c6832`](https://github.com/noctisynth/fomalhaut/commit/20c68328cbfd7f0a1aa75260f5bb6932a1b38269): Add host-resolved English and Simplified Chinese locales across configuration, session discovery, protocol snapshots, SDK validation, and both frontend themes.
- [`abd58a8`](https://github.com/noctisynth/fomalhaut/commit/abd58a88a2aec5cc4200b735860a2aa99255217c): Remove the legacy [frontend].path configuration alias and installer migration; deployments must use [themes].default.

## v0.1.0-alpha.0

### Bug Fixes

- [`d5df4ea`](https://github.com/noctisynth/fomalhaut/commit/d5df4ea3c35901d0d78661cf8d8a393a4340f36b): Share bounded AccountsService and NSS profile discovery between the greeter and locker, and expose the validated current-user avatar on every lock surface.
- [`631201b`](https://github.com/noctisynth/fomalhaut/commit/631201bd365c8bd2b6fd6ce8c90809f4d7df292e): Keep session-lock surfaces outside GtkApplication ownership and defer Rust cleanup until native destroy returns, avoiding gtk4-layer-shell 1.3.0 issue #122 on GTK 4.22 and newer.
- [`c16a3b8`](https://github.com/noctisynth/fomalhaut/commit/c16a3b879deaca0e43de9cc53a0f06d5e42b2ada): Stop remapping session-lock monitor windows through GTK after gtk4-session-lock has assigned and mapped them, preventing a GdkSurface segmentation fault during startup.
- [`0614e75`](https://github.com/noctisynth/fomalhaut/commit/0614e75402fb43402e37f969060e65ed260fb521): Stop the PAM IPC reader after its first terminal channel failure so cancellation cannot deadlock before locker power requests.

    Remove user-unit seccomp hardening that implicitly enables NoNewPrivs and prevents the configured PAM stack from executing unix_chkpwd.

- [`115bcf1`](https://github.com/noctisynth/fomalhaut/commit/115bcf1cf5e4b8cfbcbe3f7faf626c4fd22f6946): Keep the GTK application alive during asynchronous locker startup and preserve PAM helper execution in the systemd user unit.

### New Features

- [`115bcf1`](https://github.com/noctisynth/fomalhaut/commit/115bcf1cf5e4b8cfbcbe3f7faf626c4fd22f6946): Add shared or per-role display scaling and use the shared non-interactive logind backend for both greeter and locker power actions.

    Keep the session lock held while locker power requests cancel any active reauthentication transaction.

- [`a66e689`](https://github.com/noctisynth/fomalhaut/commit/a66e689814a577da88a449f4b3166daf7cb3beeb): Implement the compositor-neutral Wayland session locker with isolated PAM reauthentication, per-monitor session-lock surfaces, trusted native fallback, and systemd readiness.

    Expose shared host and controller signals required to route cross-view events and fail closed when the authentication worker becomes unavailable.
