# Changelog

## v0.1.0-alpha.2

### New Features

- [`7c3c5b0`](https://github.com/noctisynth/fomalhaut/commit/7c3c5b008ca55d29cbd0ea439903da3d29a9c959): Discover installed themes by stable manifest ID across source and package installation roots, with deterministic precedence and safe migration between local and AUR installations.

## v0.1.0-alpha.1

### New Features

- [`20c6832`](https://github.com/noctisynth/fomalhaut/commit/20c68328cbfd7f0a1aa75260f5bb6932a1b38269): Add host-resolved English and Simplified Chinese locales across configuration, session discovery, protocol snapshots, SDK validation, and both frontend themes.
- [`abd58a8`](https://github.com/noctisynth/fomalhaut/commit/abd58a88a2aec5cc4200b735860a2aa99255217c): Remove the legacy [frontend].path configuration alias and installer migration; deployments must use [themes].default.

## v0.1.0-alpha.0

### New Features

- [`115bcf1`](https://github.com/noctisynth/fomalhaut/commit/115bcf1cf5e4b8cfbcbe3f7faf626c4fd22f6946): Add shared or per-role display scaling and use the shared non-interactive logind backend for both greeter and locker power actions.

    Keep the session lock held while locker power requests cancel any active reauthentication transaction.

- [`d1828f7`](https://github.com/noctisynth/fomalhaut/commit/d1828f799cc149a9899368f7f7d625ed4d69f7aa): Add shared role-scoped configuration and theme selection.
