#!/usr/bin/env python3
"""Resolve Semifold publish facts into deterministic AUR build matrix entries."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Callable

SCHEMA_VERSION = 1
HTTP_USER_AGENT = "Fomalhaut-AUR-CI/1 (+https://github.com/noctisynth/fomalhaut)"
MAX_HTTP_BYTES = 1024 * 1024
SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[+-][0-9A-Za-z.-]+)?$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
AVAILABLE_SKIP_REASON = "registry-version-exists"
VALID_STATUSES = {"succeeded", "skipped", "failed", "not-started"}

PACKAGE_SPECS = {
    "greetd-fomalhaut": {
        "main": "fomalhaut",
        "manifest": "crates/fomalhaut/Cargo.toml",
        "dependencies": {
            "fomalhaut-config",
            "fomalhaut-core",
            "fomalhaut-greetd",
            "fomalhaut-gtk",
            "fomalhaut-logind",
            "fomalhaut-session",
            "fomalhaut-user",
            "fomalhaut-web",
        },
    },
    "fomalhaut-lock": {
        "main": "fomalhaut-lock",
        "manifest": "crates/fomalhaut-lock/Cargo.toml",
        "dependencies": {
            "fomalhaut-config",
            "fomalhaut-core",
            "fomalhaut-gtk",
            "fomalhaut-logind",
            "fomalhaut-pam",
            "fomalhaut-user",
            "fomalhaut-web",
        },
    },
}


class ResolutionError(Exception):
    """A publish fact is missing, ambiguous, or unsafe to map to AUR."""


def parse_publish_output(payload: str) -> dict:
    try:
        output = json.loads(payload)
    except json.JSONDecodeError as error:
        raise ResolutionError("Semifold publish output is not valid JSON") from error
    if not isinstance(output, dict):
        raise ResolutionError("Semifold publish output must be an object")
    if type(output.get("schema-version")) is not int or output["schema-version"] != SCHEMA_VERSION:
        raise ResolutionError("unsupported Semifold publish output schema")
    if output.get("dry-run") is not False:
        raise ResolutionError("dry-run Semifold output cannot publish AUR packages")
    packages = output.get("packages")
    if not isinstance(packages, list):
        raise ResolutionError("Semifold publish packages must be an array")
    seen: set[str] = set()
    for record in packages:
        if not isinstance(record, dict):
            raise ResolutionError("Semifold package result must be an object")
        package = record.get("package")
        version = record.get("version")
        status = record.get("status")
        if not isinstance(package, str) or not package:
            raise ResolutionError("Semifold package result has an invalid package ID")
        if package in seen:
            raise ResolutionError(f"Semifold package result is duplicated: {package}")
        seen.add(package)
        if not isinstance(version, str) or not SEMVER.fullmatch(version):
            raise ResolutionError(f"Semifold package has an invalid version: {package}")
        if status not in VALID_STATUSES:
            raise ResolutionError(f"Semifold package has an unknown status: {package}")
        for field in ("skip-reason", "failure-stage"):
            value = record.get(field)
            if value is not None and not isinstance(value, str):
                raise ResolutionError(f"Semifold package has an invalid {field}: {package}")
    return output


def is_main_available(record: dict | None) -> bool:
    if record is None:
        return False
    return record["status"] == "succeeded" or (
        record["status"] == "skipped"
        and record.get("skip-reason") == AVAILABLE_SKIP_REASON
    )


def is_published(record: dict | None) -> bool:
    return record is not None and record["status"] == "succeeded"


def split_aur_version(version: str) -> tuple[str, int]:
    pkgver, separator, pkgrel = version.rpartition("-")
    if not separator or not pkgver or not pkgrel.isascii() or not pkgrel.isdigit():
        raise ResolutionError(f"AUR version does not use an integer pkgrel: {version}")
    release = int(pkgrel)
    if release < 1:
        raise ResolutionError(f"AUR pkgrel is not positive: {version}")
    return pkgver, release


def matrix_entry(
    aur_package: str, upstream_version: str, pkgrel: int, source_ref: str
) -> dict:
    if not SEMVER.fullmatch(upstream_version):
        raise ResolutionError(f"unsupported upstream version: {upstream_version}")
    if pkgrel < 1:
        raise ResolutionError("pkgrel must be positive")
    if not COMMIT.fullmatch(source_ref):
        raise ResolutionError("source ref must be a complete lowercase commit SHA")
    return {
        "aur_package": aur_package,
        "upstream_version": upstream_version,
        "pkgver": upstream_version.replace("-", "."),
        "pkgrel": str(pkgrel),
        "source_ref": source_ref,
    }


def resolve_automatic(
    payload: str,
    source_ref: str,
    manifest_versions: dict[str, str],
    aur_versions: dict[str, str | None],
) -> dict:
    output = parse_publish_output(payload)
    records = {record["package"]: record for record in output["packages"]}
    entries = []
    for aur_package, spec in PACKAGE_SPECS.items():
        main_package = spec["main"]
        main_record = records.get(main_package)
        main_available = is_main_available(main_record)
        dependency_published = any(
            is_published(records.get(dependency)) for dependency in spec["dependencies"]
        )
        if not main_available and not dependency_published:
            continue

        upstream_version = manifest_versions.get(main_package)
        if not isinstance(upstream_version, str) or not SEMVER.fullmatch(upstream_version):
            raise ResolutionError(f"invalid manifest version for {main_package}")
        if main_available and main_record["version"] != upstream_version:
            raise ResolutionError(
                f"Semifold version {main_record['version']} does not match "
                f"{main_package} manifest {upstream_version}"
            )

        current = aur_versions.get(aur_package)
        target_pkgver = upstream_version.replace("-", ".")
        if main_available and (
            current is None or split_aur_version(current)[0] != target_pkgver
        ):
            entries.append(matrix_entry(aur_package, upstream_version, 1, source_ref))
            continue
        if main_record is not None and main_record["status"] == "succeeded":
            continue
        if not dependency_published:
            continue
        if current is None:
            raise ResolutionError(
                f"{aur_package} does not exist; a dependency-only release cannot create it"
            )
        current_pkgver, current_pkgrel = split_aur_version(current)
        if current_pkgver != target_pkgver:
            raise ResolutionError(
                f"{aur_package} version {current} cannot accept a dependency rebuild "
                f"for {target_pkgver}"
            )
        entries.append(
            matrix_entry(aur_package, upstream_version, current_pkgrel + 1, source_ref)
        )
    return {"include": entries}


def affected_aur_packages(payload: str) -> list[str]:
    output = parse_publish_output(payload)
    records = {record["package"]: record for record in output["packages"]}
    return [
        aur_package
        for aur_package, spec in PACKAGE_SPECS.items()
        if is_main_available(records.get(spec["main"]))
        or any(
            is_published(records.get(dependency))
            for dependency in spec["dependencies"]
        )
    ]


def resolve_manual(
    aur_package: str,
    upstream_version: str,
    source_ref: str,
    requested_pkgrel: int,
    current: str | None,
) -> dict:
    if aur_package not in PACKAGE_SPECS:
        raise ResolutionError(f"unsupported AUR package: {aur_package}")
    entry = matrix_entry(aur_package, upstream_version, requested_pkgrel, source_ref)
    if current is not None:
        current_pkgver, current_pkgrel = split_aur_version(current)
        if current_pkgver == entry["pkgver"] and requested_pkgrel <= current_pkgrel:
            raise ResolutionError(
                f"manual pkgrel {requested_pkgrel} must exceed current {current_pkgrel}"
            )
        if current_pkgver != entry["pkgver"] and requested_pkgrel != 1:
            raise ResolutionError("a new pkgver must begin at pkgrel 1")
    elif requested_pkgrel != 1:
        raise ResolutionError("a new AUR package must begin at pkgrel 1")
    return {"include": [entry]}


def git(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=repository,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        raise ResolutionError("Git could not resolve the requested immutable source")
    return result.stdout.strip()


def resolve_commit(repository: Path, reference: str) -> str:
    commit = git(repository, "rev-parse", "--verify", f"{reference}^{{commit}}")
    if not COMMIT.fullmatch(commit):
        raise ResolutionError("source ref did not resolve to a complete commit SHA")
    return commit


def manifest_version(repository: Path, commit: str, manifest: str) -> str:
    content = git(repository, "show", f"{commit}:{manifest}")
    for line in content.splitlines():
        match = re.fullmatch(r'version = "([^"]+)"', line)
        if match:
            version = match.group(1)
            if SEMVER.fullmatch(version):
                return version
            break
    raise ResolutionError(f"manifest has no supported package version: {manifest}")


def fetch_aur_version(package: str) -> str | None:
    url = "https://aur.archlinux.org/rpc/v5/info/" + urllib.parse.quote(
        package, safe=""
    )
    request = urllib.request.Request(url, headers={"User-Agent": HTTP_USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            body = response.read(MAX_HTTP_BYTES + 1)
    except (OSError, urllib.error.URLError) as error:
        raise ResolutionError(f"AUR RPC request failed for {package}") from error
    if len(body) > MAX_HTTP_BYTES:
        raise ResolutionError("AUR RPC response exceeded 1 MiB")
    try:
        payload = json.loads(body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ResolutionError("AUR RPC returned an invalid response") from error
    if not isinstance(payload, dict) or type(payload.get("resultcount")) is not int:
        raise ResolutionError("AUR RPC response has an invalid envelope")
    if payload["resultcount"] == 0:
        return None
    results = payload.get("results")
    if payload["resultcount"] != 1 or not isinstance(results, list) or len(results) != 1:
        raise ResolutionError(f"AUR RPC returned an ambiguous result for {package}")
    result = results[0]
    if not isinstance(result, dict) or result.get("Name") != package:
        raise ResolutionError(f"AUR RPC returned the wrong package for {package}")
    version = result.get("Version")
    if not isinstance(version, str):
        raise ResolutionError(f"AUR RPC omitted the version for {package}")
    split_aur_version(version)
    return version


def collect_manifest_versions(repository: Path, commit: str) -> dict[str, str]:
    return {
        spec["main"]: manifest_version(repository, commit, spec["manifest"])
        for spec in PACKAGE_SPECS.values()
    }


def collect_aur_versions(
    fetch: Callable[[str], str | None], packages: list[str]
) -> dict[str, str | None]:
    return {package: fetch(package) for package in packages}


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument("--repository", type=Path, default=Path.cwd())
    subcommands = result.add_subparsers(dest="command", required=True)
    automatic = subcommands.add_parser("automatic")
    automatic.add_argument("--publish-json", required=True)
    automatic.add_argument("--source-sha", required=True)
    manual = subcommands.add_parser("manual")
    manual.add_argument("--package", choices=sorted(PACKAGE_SPECS), required=True)
    manual.add_argument("--source-ref", required=True)
    manual.add_argument("--pkgrel", type=int, required=True)
    return result


def main() -> int:
    arguments = parser().parse_args()
    repository = arguments.repository.resolve()
    try:
        if arguments.command == "automatic":
            if not COMMIT.fullmatch(arguments.source_sha):
                raise ResolutionError("Semifold source SHA must be a complete commit SHA")
            source_ref = resolve_commit(repository, arguments.source_sha)
            if source_ref != arguments.source_sha:
                raise ResolutionError("Semifold source SHA did not resolve exactly")
            matrix = resolve_automatic(
                arguments.publish_json,
                source_ref,
                collect_manifest_versions(repository, source_ref),
                collect_aur_versions(
                    fetch_aur_version,
                    affected_aur_packages(arguments.publish_json),
                ),
            )
        else:
            source_ref = resolve_commit(repository, arguments.source_ref)
            spec = PACKAGE_SPECS[arguments.package]
            upstream_version = manifest_version(
                repository, source_ref, spec["manifest"]
            )
            matrix = resolve_manual(
                arguments.package,
                upstream_version,
                source_ref,
                arguments.pkgrel,
                fetch_aur_version(arguments.package),
            )
    except ResolutionError as error:
        print(f"AUR resolution failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(matrix, separators=(",", ":"), sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
