# Changelog

## v0.1.0-alpha.0

### Bug Fixes

- [`0614e75`](https://github.com/noctisynth/fomalhaut/commit/0614e75402fb43402e37f969060e65ed260fb521): Stop the PAM IPC reader after its first terminal channel failure so cancellation cannot deadlock before locker power requests.

    Remove user-unit seccomp hardening that implicitly enables NoNewPrivs and prevents the configured PAM stack from executing unix_chkpwd.

- [`115bcf1`](https://github.com/noctisynth/fomalhaut/commit/115bcf1cf5e4b8cfbcbe3f7faf626c4fd22f6946): Send terminal PAM outcomes only after context cleanup so ordinary authentication rejection remains recoverable in the Web UI.

    Classify bounded worker shutdown failures without exposing PAM or credential details.


### Chores

- [`6949b64`](https://github.com/noctisynth/fomalhaut/commit/6949b64c1ef6558bde0425198f715b79f676abe4): Add the audited PAM client dependency without its CLI feature.

### New Features

- [`a66e689`](https://github.com/noctisynth/fomalhaut/commit/a66e689814a577da88a449f4b3166daf7cb3beeb): Implement the compositor-neutral Wayland session locker with isolated PAM reauthentication, per-monitor session-lock surfaces, trusted native fallback, and systemd readiness.

    Expose shared host and controller signals required to route cross-view events and fail closed when the authentication worker becomes unavailable.

