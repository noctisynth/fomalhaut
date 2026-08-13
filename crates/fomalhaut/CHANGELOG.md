# Changelog

## v0.1.0-alpha.3

### Bug Fixes

- [`3fec55b`](https://github.com/noctisynth/fomalhaut/commit/3fec55b78678b0eb8ba8bba584de54d7d7d2eeec): Clean up the failed greetd configuration slot after authentication errors while treating an already-exited PAM worker response as a recoverable rejection.
- [`7b061a8`](https://github.com/noctisynth/fomalhaut/commit/7b061a8b36315eadfe23cf0bce74c9bae99c208b): Treat greetd authentication errors as already-cancelled failures so the greeter clears stale prompts and can retry without sending a redundant CancelSession request.
- [`27c5ee8`](https://github.com/noctisynth/fomalhaut/commit/27c5ee8e57a99546534a39069edc09585497ec45): Normalize standard password prompts to the active theme locale in both greeter and locker while preserving OTP, PIN, visible, and custom PAM prompts.
- [`2c274c9`](https://github.com/noctisynth/fomalhaut/commit/2c274c901a5ec73d79c45755a77f2a1ce882d99b): Restart locker authentication with a fresh PAM transaction after system resume and prevent themes from submitting prompts cancelled before suspend.

### New Features

- [`20c6832`](https://github.com/noctisynth/fomalhaut/commit/20c68328cbfd7f0a1aa75260f5bb6932a1b38269): Add host-resolved English and Simplified Chinese locales across configuration, session discovery, protocol snapshots, SDK validation, and both frontend themes.
- [`abd58a8`](https://github.com/noctisynth/fomalhaut/commit/abd58a88a2aec5cc4200b735860a2aa99255217c): Remove the legacy [frontend].path configuration alias and installer migration; deployments must use [themes].default.

## v0.1.0-alpha.2

### Bug Fixes

- [`d5df4ea`](https://github.com/noctisynth/fomalhaut/commit/d5df4ea3c35901d0d78661cf8d8a393a4340f36b): Share bounded AccountsService and NSS profile discovery between the greeter and locker, and expose the validated current-user avatar on every lock surface.

### New Features

- [`a0dc068`](https://github.com/noctisynth/fomalhaut/commit/a0dc068969a9f16459122cca953b924812364daa): Add the role-discriminated greeter and locker protocol, capability-separated controllers, and generated schema bindings.

    Bootstrap the shared TypeScript SDK into mode-generic clients and update both bundled themes for single-page greeter and locker support.

- [`115bcf1`](https://github.com/noctisynth/fomalhaut/commit/115bcf1cf5e4b8cfbcbe3f7faf626c4fd22f6946): Add shared or per-role display scaling and use the shared non-interactive logind backend for both greeter and locker power actions.

    Keep the session lock held while locker power requests cancel any active reauthentication transaction.

- [`a2ffddb`](https://github.com/noctisynth/fomalhaut/commit/a2ffddb00d5b5512b04c50a8df0fe11db273f16e): Extract the shared GTK4 and WebKitGTK host infrastructure.
- [`d1828f7`](https://github.com/noctisynth/fomalhaut/commit/d1828f799cc149a9899368f7f7d625ed4d69f7aa): Add shared role-scoped configuration and theme selection.

### Refactors

- [`e3dac89`](https://github.com/noctisynth/fomalhaut/commit/e3dac89f49692b4742e6b5e10b1884e71c684e8f): Split backend-neutral authentication types from the greetd login backend.

## v0.1.0-alpha.1

### Bug Fixes

- [`fadd8c8`](https://github.com/noctisynth/fomalhaut/commit/fadd8c80dd9e79d7f9e6b019276476aad45ec560): Fix authentication retry cleanup, stabilize login navigation, and use the Luma Select for sessions.

### New Features

- [`f6bf099`](https://github.com/noctisynth/fomalhaut/commit/f6bf0992cb9551d7c90a92e0b17cb13aedf6968c): Add validated fractional WebKit page scaling for HiDPI greeter displays.
- [`feaf0b0`](https://github.com/noctisynth/fomalhaut/commit/feaf0b03c7e061c8733e6fbf306c552588fbc1a4): Add policy-gated systemd-logind power controls and capability-aware frontend actions.
- [`ec9489e`](https://github.com/noctisynth/fomalhaut/commit/ec9489e1a36214257a6652e12a27b40a70d090ba): Add trusted user discovery, bounded avatar resources, and typed frontend user summaries.

## v0.1.0-alpha.0

### Bug Fixes

- [`2539c27`](https://github.com/noctisynth/fomalhaut/commit/2539c2726b8da516c9f856bf6c24548eafd730d2): Add required registry metadata and build the TypeScript SDK before Semifold publishes package artifacts.
- [`13ae9b0`](https://github.com/noctisynth/fomalhaut/commit/13ae9b02399d626ce86cc841c730b1de44d3ea1f): Make page context invalidation and WebView load transitions observable, and document runtime verification of navigation, popup, download, remote resource, refresh, and renderer crash handling.

### Chores

- [`b4089d9`](https://github.com/noctisynth/fomalhaut/commit/b4089d93828cf3d6b9b0780af2444b91023131bb): Prepare every workspace crate for the initial alpha release.

### New Features

- [`d413148`](https://github.com/noctisynth/fomalhaut/commit/d413148f6557a508b73ee3146742979116413d42): Replace the embedded bridge probe with an accessible, framework-free minimal login theme supporting trusted session selection, usernames, arbitrary PAM prompts, cancellation, bounded messages, busy states, and immediate credential input clearing.
- [`777d95a`](https://github.com/noctisynth/fomalhaut/commit/777d95acf40e5d5284e1ecd2a065deedeacd1dfb): Connect the WebKitGTK bridge to a toolkit-independent authentication controller backed by the real greetd IPC transport, with bounded worker channels, page lifecycle isolation, sanitized failures, and stub coverage.
- [`c83b998`](https://github.com/noctisynth/fomalhaut/commit/c83b9985975f8e4275cbd67d0071b82e8236544f): Add strict system configuration, capability-confined external theme loading, and configurable trusted session discovery paths.
- [`184d0c6`](https://github.com/noctisynth/fomalhaut/commit/184d0c611786a65972478d5fb3f10097fe357dbe): Discover trusted desktop sessions, expose only bounded session metadata, validate frontend selection against the host catalog, automatically start the selected greetd session after authentication, and exit the WebKitGTK host on successful handoff.
- [`ef12f1b`](https://github.com/noctisynth/fomalhaut/commit/ef12f1b6544f6b9567950eeb4aee3c091b701b26): Add the native GTK4 and WebKitGTK host prototype with an isolated custom resource scheme, a protocol v1 bridge, hardened WebView policies, and embedded validation assets.
