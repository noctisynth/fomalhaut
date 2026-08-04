#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Fomalhaut contributors
# SPDX-License-Identifier: 0BSD

set -euo pipefail

usage() {
  echo "usage: $0 <tag> <pkgrel> <source-sha256> <maintainer-name> <maintainer-email> <output-dir>" >&2
}

if [[ $# -ne 6 ]]; then
  usage
  exit 2
fi

tag=$1
pkgrel=$2
source_sha256=$3
maintainer_name=$4
maintainer_email=$5
output_dir=$6

if [[ ! ${tag} =~ ^fomalhaut-v([0-9A-Za-z.+-]+)$ ]]; then
  echo "invalid Fomalhaut release tag: ${tag}" >&2
  exit 2
fi

upstream_version=${BASH_REMATCH[1]}
if [[ ! ${upstream_version} =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  echo "tag does not contain a supported SemVer version: ${tag}" >&2
  exit 2
fi

if [[ ! ${pkgrel} =~ ^[1-9][0-9]*$ ]]; then
  echo "pkgrel must be a positive integer" >&2
  exit 2
fi

if [[ ! ${source_sha256} =~ ^[0-9a-f]{64}$ ]]; then
  echo "source SHA-256 must contain 64 lowercase hexadecimal characters" >&2
  exit 2
fi

if [[ -z ${maintainer_name} || ${maintainer_name} == *$'\n'* || ${maintainer_name} == *$'\r'* ]]; then
  echo "maintainer name must be non-empty and single-line" >&2
  exit 2
fi

if [[ ! ${maintainer_email} =~ ^[^\<\>[:space:]]+@[^\<\>[:space:]]+$ ]]; then
  echo "maintainer email is invalid" >&2
  exit 2
fi

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
mkdir -p -- "${output_dir}"

config_sha256=$(sha256sum "${script_dir}/greetd-fomalhaut.toml")
config_sha256=${config_sha256%% *}
pkgver=${upstream_version//-/.}

escape_sed_replacement() {
  sed -e 's/[\\&|]/\\&/g' <<<"$1"
}

escaped_upstream_version=$(escape_sed_replacement "${upstream_version}")
escaped_pkgver=$(escape_sed_replacement "${pkgver}")
escaped_pkgrel=$(escape_sed_replacement "${pkgrel}")
escaped_source_sha256=$(escape_sed_replacement "${source_sha256}")
escaped_config_sha256=$(escape_sed_replacement "${config_sha256}")
escaped_maintainer_name=$(escape_sed_replacement "${maintainer_name}")
escaped_maintainer_email=$(escape_sed_replacement "${maintainer_email}")

sed \
  -e "s|@UPSTREAM_VERSION@|${escaped_upstream_version}|g" \
  -e "s|@PKGVER@|${escaped_pkgver}|g" \
  -e "s|@PKGREL@|${escaped_pkgrel}|g" \
  -e "s|@SOURCE_SHA256@|${escaped_source_sha256}|g" \
  -e "s|@CONFIG_SHA256@|${escaped_config_sha256}|g" \
  -e "s|@MAINTAINER_NAME@|${escaped_maintainer_name}|g" \
  -e "s|@MAINTAINER_EMAIL@|${escaped_maintainer_email}|g" \
  "${script_dir}/PKGBUILD.in" >"${output_dir}/PKGBUILD"

if grep -Eq '@[A-Z0-9_]+@' "${output_dir}/PKGBUILD"; then
  echo "rendered PKGBUILD still contains template placeholders" >&2
  exit 1
fi

install -m 644 "${script_dir}/greetd-fomalhaut.toml" "${output_dir}/greetd-fomalhaut.toml"
install -m 644 "${script_dir}/LICENSE" "${output_dir}/LICENSE"
