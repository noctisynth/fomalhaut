# Changelog

## v0.1.0-alpha.0

### Bug Fixes

- [`2539c27`](https://github.com/noctisynth/fomalhaut/commit/2539c2726b8da516c9f856bf6c24548eafd730d2): Add required registry metadata and build the TypeScript SDK before Semifold publishes package artifacts.

### Chores

- [`b4089d9`](https://github.com/noctisynth/fomalhaut/commit/b4089d93828cf3d6b9b0780af2444b91023131bb): Prepare every workspace crate for the initial alpha release.

### New Features

- [`2057319`](https://github.com/noctisynth/fomalhaut/commit/20573197ab1963843fae2294be8587f6cdf72638): Implement the UI-independent greetd authentication state machine, Unix socket transport,
    sensitive data handling, and comprehensive Core tests.
- [`184d0c6`](https://github.com/noctisynth/fomalhaut/commit/184d0c611786a65972478d5fb3f10097fe357dbe): Discover trusted desktop sessions, expose only bounded session metadata, validate frontend selection against the host catalog, automatically start the selected greetd session after authentication, and exit the WebKitGTK host on successful handoff.
