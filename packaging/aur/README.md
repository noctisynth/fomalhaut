# Fomalhaut AUR packaging

<!-- SPDX-FileCopyrightText: 2026 Fomalhaut contributors -->
<!-- SPDX-License-Identifier: 0BSD -->

This directory is the source for the independently maintained
`greetd-fomalhaut`, `fomalhaut-lock`, and `fomalhaut-theme-nocturne` AUR
repositories. Each repository receives its rendered `PKGBUILD`, `.SRCINFO`,
and packaging `LICENSE`; `greetd-fomalhaut` also receives the example greetd
configuration and pacman install message. The templates, renderer, and this
README remain in the upstream repository.

The packaging metadata is licensed under 0BSD so it can follow Arch packaging
policy. Fomalhaut itself remains AGPL-3.0-only, and the generated `PKGBUILD`
declares that upstream license.

The `AUR` GitHub Actions workflow needs these repository variables:

- `AUR_MAINTAINER_NAME`
- `AUR_MAINTAINER_EMAIL`

The protected `aur-production` environment needs these secrets:

- `AUR_SSH_PRIVATE_KEY`: a dedicated, revocable key registered with the AUR
  account.

The workflow obtains the current `aur.archlinux.org` Ed25519 host key with
`ssh-keyscan`, then verifies that its unique SHA-256 fingerprint matches the
official fingerprint pinned in the reviewed workflow before using it as
`known_hosts`. A host-key rotation therefore fails closed until the new
fingerprint is independently verified and committed; no known-hosts secret is
needed.

Configure required reviewers on `aur-production`. Automatic runs consume the
schema-v1 `semifold-publish` output from the calling Semifold CI workflow. A
main package release synchronizes the corresponding AUR `pkgver`; a release of
only a binary dependency rebuilds the same `pkgver` with the next integer
`pkgrel`. The exact private-package skip for `@fomalhaut/theme-nocturne`
synchronizes the theme AUR version without publishing the theme to npm. The
source archive is pinned to the Semifold publish commit, and the workflow no
longer probes registries or infers releases from tags.

The theme package uses its isolated npm build manifest and lockfile with
`npm ci`; this avoids relying on Arch's stable Bun for a project developed with
an incompatible Bun canary. npm and Node.js are build-only dependencies. The
installed package contains only static files below
`/usr/share/fomalhaut/themes/nocturne`; select it in Fomalhaut configuration
with the stable theme ID `nocturne`.

Use the workflow's manual package, immutable source ref, and `pkgrel` inputs
for packaging-only revisions. A revision of an existing `pkgver` must increase
the current AUR `pkgrel`.
