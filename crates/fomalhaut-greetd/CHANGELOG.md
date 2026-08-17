# Changelog

## v0.1.0-alpha.1

### Bug Fixes

- [`3fec55b`](https://github.com/noctisynth/fomalhaut/commit/3fec55b78678b0eb8ba8bba584de54d7d7d2eeec): Clean up the failed greetd configuration slot after authentication errors while treating an already-exited PAM worker response as a recoverable rejection.
- [`7b061a8`](https://github.com/noctisynth/fomalhaut/commit/7b061a8b36315eadfe23cf0bce74c9bae99c208b): Treat greetd authentication errors as already-cancelled failures so the greeter clears stale prompts and can retry without sending a redundant CancelSession request.

## v0.1.0-alpha.0

### Refactors

- [`e3dac89`](https://github.com/noctisynth/fomalhaut/commit/e3dac89f49692b4742e6b5e10b1884e71c684e8f): Split backend-neutral authentication types from the greetd login backend.
