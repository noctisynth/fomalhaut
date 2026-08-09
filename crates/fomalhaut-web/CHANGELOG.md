# Changelog

## v0.1.0-alpha.2

### New Features

- [`a0dc068`](https://github.com/noctisynth/fomalhaut/commit/a0dc068969a9f16459122cca953b924812364daa): Add the role-discriminated greeter and locker protocol, capability-separated controllers, and generated schema bindings.

    Bootstrap the shared TypeScript SDK into mode-generic clients and update both bundled themes for single-page greeter and locker support.

- [`115bcf1`](https://github.com/noctisynth/fomalhaut/commit/115bcf1cf5e4b8cfbcbe3f7faf626c4fd22f6946): Add shared or per-role display scaling and use the shared non-interactive logind backend for both greeter and locker power actions.

    Keep the session lock held while locker power requests cancel any active reauthentication transaction.

- [`a66e689`](https://github.com/noctisynth/fomalhaut/commit/a66e689814a577da88a449f4b3166daf7cb3beeb): Implement the compositor-neutral Wayland session locker with isolated PAM reauthentication, per-monitor session-lock surfaces, trusted native fallback, and systemd readiness.

    Expose shared host and controller signals required to route cross-view events and fail closed when the authentication worker becomes unavailable.


### Refactors

- [`e3dac89`](https://github.com/noctisynth/fomalhaut/commit/e3dac89f49692b4742e6b5e10b1884e71c684e8f): Split backend-neutral authentication types from the greetd login backend.

## v0.1.0-alpha.1

### Bug Fixes

- [`fadd8c8`](https://github.com/noctisynth/fomalhaut/commit/fadd8c80dd9e79d7f9e6b019276476aad45ec560): Fix authentication retry cleanup, stabilize login navigation, and use the Luma Select for sessions.

### New Features

- [`feaf0b0`](https://github.com/noctisynth/fomalhaut/commit/feaf0b03c7e061c8733e6fbf306c552588fbc1a4): Add policy-gated systemd-logind power controls and capability-aware frontend actions.
- [`ec9489e`](https://github.com/noctisynth/fomalhaut/commit/ec9489e1a36214257a6652e12a27b40a70d090ba): Add trusted user discovery, bounded avatar resources, and typed frontend user summaries.

## v0.1.0-alpha.0

### Bug Fixes

- [`2539c27`](https://github.com/noctisynth/fomalhaut/commit/2539c2726b8da516c9f856bf6c24548eafd730d2): Add required registry metadata and build the TypeScript SDK before Semifold publishes package artifacts.

### Chores

- [`a691626`](https://github.com/noctisynth/fomalhaut/commit/a69162609d734f4888af9da97ccf514ac439e874): Define frontend protocol v1 with strict bounded request decoding, zeroizing authentication responses, sequenced events, state snapshots, and generated JSON Schema.
- [`b4089d9`](https://github.com/noctisynth/fomalhaut/commit/b4089d93828cf3d6b9b0780af2444b91023131bb): Prepare every workspace crate for the initial alpha release.

### New Features

- [`d413148`](https://github.com/noctisynth/fomalhaut/commit/d413148f6557a508b73ee3146742979116413d42): Replace the embedded bridge probe with an accessible, framework-free minimal login theme supporting trusted session selection, usernames, arbitrary PAM prompts, cancellation, bounded messages, busy states, and immediate credential input clearing.
- [`777d95a`](https://github.com/noctisynth/fomalhaut/commit/777d95acf40e5d5284e1ecd2a065deedeacd1dfb): Connect the WebKitGTK bridge to a toolkit-independent authentication controller backed by the real greetd IPC transport, with bounded worker channels, page lifecycle isolation, sanitized failures, and stub coverage.
- [`c83b998`](https://github.com/noctisynth/fomalhaut/commit/c83b9985975f8e4275cbd67d0071b82e8236544f): Add strict system configuration, capability-confined external theme loading, and configurable trusted session discovery paths.
- [`184d0c6`](https://github.com/noctisynth/fomalhaut/commit/184d0c611786a65972478d5fb3f10097fe357dbe): Discover trusted desktop sessions, expose only bounded session metadata, validate frontend selection against the host catalog, automatically start the selected greetd session after authentication, and exit the WebKitGTK host on successful handoff.
- [`af8105c`](https://github.com/noctisynth/fomalhaut/commit/af8105c738f1afc73bc67ac6e76f468db985ef0a): Generate module-aligned TypeScript protocol bindings and provide a typed WebKit client SDK with Bun-based validation and tests.
- [`ef12f1b`](https://github.com/noctisynth/fomalhaut/commit/ef12f1b6544f6b9567950eeb4aee3c091b701b26): Add the native GTK4 and WebKitGTK host prototype with an isolated custom resource scheme, a protocol v1 bridge, hardened WebView policies, and embedded validation assets.
