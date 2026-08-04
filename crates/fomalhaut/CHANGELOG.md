# Changelog

## v0.1.0-alpha.0

### Bug Fixes

- [`13ae9b0`](https://github.com/noctisynth/fomalhaut/commit/13ae9b02399d626ce86cc841c730b1de44d3ea1f): Make page context invalidation and WebView load transitions observable, and document runtime verification of navigation, popup, download, remote resource, refresh, and renderer crash handling.

### Chores

- [`b4089d9`](https://github.com/noctisynth/fomalhaut/commit/b4089d93828cf3d6b9b0780af2444b91023131bb): Prepare every workspace crate for the initial alpha release.

### New Features

- [`d413148`](https://github.com/noctisynth/fomalhaut/commit/d413148f6557a508b73ee3146742979116413d42): Replace the embedded bridge probe with an accessible, framework-free minimal login theme supporting trusted session selection, usernames, arbitrary PAM prompts, cancellation, bounded messages, busy states, and immediate credential input clearing.
- [`777d95a`](https://github.com/noctisynth/fomalhaut/commit/777d95acf40e5d5284e1ecd2a065deedeacd1dfb): Connect the WebKitGTK bridge to a toolkit-independent authentication controller backed by the real greetd IPC transport, with bounded worker channels, page lifecycle isolation, sanitized failures, and stub coverage.
- [`c83b998`](https://github.com/noctisynth/fomalhaut/commit/c83b9985975f8e4275cbd67d0071b82e8236544f): Add strict system configuration, capability-confined external theme loading, and configurable trusted session discovery paths.
- [`184d0c6`](https://github.com/noctisynth/fomalhaut/commit/184d0c611786a65972478d5fb3f10097fe357dbe): Discover trusted desktop sessions, expose only bounded session metadata, validate frontend selection against the host catalog, automatically start the selected greetd session after authentication, and exit the WebKitGTK host on successful handoff.
- [`ef12f1b`](https://github.com/noctisynth/fomalhaut/commit/ef12f1b6544f6b9567950eeb4aee3c091b701b26): Add the native GTK4 and WebKitGTK host prototype with an isolated custom resource scheme, a protocol v1 bridge, hardened WebView policies, and embedded validation assets.
