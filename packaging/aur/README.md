# greetd-fomalhaut AUR packaging

<!-- SPDX-FileCopyrightText: 2026 Fomalhaut contributors -->
<!-- SPDX-License-Identifier: 0BSD -->

This directory is the source for the independently maintained
`greetd-fomalhaut` AUR repository. The rendered `PKGBUILD`, `.SRCINFO`, example
greetd configuration, and packaging `LICENSE` are published to AUR; the
template, renderer, and this README remain in the upstream repository.

The packaging metadata is licensed under 0BSD so it can follow Arch packaging
policy. Fomalhaut itself remains AGPL-3.0-only, and the generated `PKGBUILD`
declares that upstream license.

The `AUR` GitHub Actions workflow needs these repository variables:

- `AUR_MAINTAINER_NAME`
- `AUR_MAINTAINER_EMAIL`

The protected `aur-production` environment needs these secrets:

- `AUR_SSH_PRIVATE_KEY`: a dedicated, revocable key registered with the AUR
  account.
- `AUR_SSH_KNOWN_HOSTS`: administrator-verified OpenSSH known-hosts entries for
  `aur.archlinux.org`.

Configure required reviewers on `aur-production`. Automatic runs publish a new
`fomalhaut-v*` application tag with `pkgrel=1`. Use the workflow's manual `tag`
and `pkgrel` inputs for packaging-only revisions.
