# Changelog

## v0.1.0-alpha.0

### Bug Fixes

- [`d5df4ea`](https://github.com/noctisynth/fomalhaut/commit/d5df4ea3c35901d0d78661cf8d8a393a4340f36b): Share bounded AccountsService and NSS profile discovery between the greeter and locker, and expose the validated current-user avatar on every lock surface.

### New Features

- [`a0dc068`](https://github.com/noctisynth/fomalhaut/commit/a0dc068969a9f16459122cca953b924812364daa): Add the role-discriminated greeter and locker protocol, capability-separated controllers, and generated schema bindings.

    Bootstrap the shared TypeScript SDK into mode-generic clients and update both bundled themes for single-page greeter and locker support.

- [`a2ffddb`](https://github.com/noctisynth/fomalhaut/commit/a2ffddb00d5b5512b04c50a8df0fe11db273f16e): Extract the shared GTK4 and WebKitGTK host infrastructure.
- [`a66e689`](https://github.com/noctisynth/fomalhaut/commit/a66e689814a577da88a449f4b3166daf7cb3beeb): Implement the compositor-neutral Wayland session locker with isolated PAM reauthentication, per-monitor session-lock surfaces, trusted native fallback, and systemd readiness.

    Expose shared host and controller signals required to route cross-view events and fail closed when the authentication worker becomes unavailable.

