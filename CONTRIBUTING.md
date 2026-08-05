# Contributing to Fomalhaut

Thank you for helping improve Fomalhaut. Bug reports, design discussions,
documentation, themes, tests, and code contributions are all welcome.

## Before you start

Check [TODO.md](TODO.md) and the issue tracker to see whether the work is already
planned or in progress. For substantial features, protocol changes, or changes
to a security boundary, open a discussion or issue before investing in an
implementation.

The [technical design](docs/DESIGN.md) describes the architecture and trust
model. The [configuration guide](docs/CONFIGURATION.md) covers installation,
system configuration, and external themes.

## Development environment

The project uses the latest stable Rust toolchain and Bun canary. Building the
complete application also requires the GTK 4 and WebKitGTK 6.0 development
libraries; the platform requirements are listed in the
[installation guide](docs/CONFIGURATION.md).

Clone the repository and install the JavaScript workspace dependencies:

```sh
git clone https://github.com/noctisynth/fomalhaut.git
cd fomalhaut
bun install --frozen-lockfile
```

Build the Rust workspace with:

```sh
cargo build --workspace
```

Build the Nocturne reference theme with:

```sh
bun run build:theme
```

## Checks

Run the relevant checks before opening a pull request. For Rust changes:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used
cargo test --workspace --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

For the TypeScript SDK and reference theme:

```sh
bun run check:sdk
bun run --cwd packages/fomalhaut-sdk test
bun run check:theme
bun run test:theme
bun run build:theme
```

The continuous integration workflow is the source of truth for the complete
check suite.

## Changesets

Changes that affect a publishable crate or the TypeScript SDK normally need a
Semifold changeset. Use `smif status` to inspect the current release state and
`smif commit` to create the changeset included with your pull request. Package
versioning and publishing are handled by CI.

Pure documentation, CI, packaging, and repository-maintenance changes usually
do not require a changeset.

## Pull requests

- Keep each pull request focused and explain the user-visible behavior and the
  reason for the change.
- Add tests for new behavior and regressions where practical.
- Update user documentation when installation, configuration, or theme behavior
  changes.
- Keep [docs/DESIGN.md](docs/DESIGN.md) and [TODO.md](TODO.md) aligned with
  architectural decisions and implementation status.
- Do not include build artifacts, credentials, or unrelated formatting changes.

By contributing, you agree that your contribution is licensed under the
project's [AGPL-3.0-only license](LICENSE).
