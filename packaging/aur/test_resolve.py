import importlib.util
import json
import sys
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("resolve.py")
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("fomalhaut_aur_resolve", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("test could not load AUR resolver")
resolver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(resolver)

SOURCE = "a" * 40
MANIFESTS = {"fomalhaut": "1.2.0-alpha.1", "fomalhaut-lock": "0.3.0-alpha.2"}
AUR = {
    "greetd-fomalhaut": "1.2.0.alpha.1-3",
    "fomalhaut-lock": "0.3.0.alpha.2-2",
}


def output(*packages):
    return json.dumps(
        {"schema-version": 1, "dry-run": False, "packages": list(packages)}
    )


def package(name, version, status="succeeded", **extra):
    return {"package": name, "version": version, "status": status, **extra}


class AutomaticResolutionTests(unittest.TestCase):
    def test_main_releases_synchronize_independent_aur_versions(self):
        payload = output(
            package("fomalhaut", "1.3.0-alpha.1"),
            package("fomalhaut-lock", "0.4.0-alpha.1"),
        )
        manifests = {"fomalhaut": "1.3.0-alpha.1", "fomalhaut-lock": "0.4.0-alpha.1"}
        matrix = resolver.resolve_automatic(payload, SOURCE, manifests, AUR)
        self.assertEqual(
            [(entry["aur_package"], entry["pkgver"], entry["pkgrel"]) for entry in matrix["include"]],
            [
                ("greetd-fomalhaut", "1.3.0.alpha.1", "1"),
                ("fomalhaut-lock", "0.4.0.alpha.1", "1"),
            ],
        )

    def test_shared_dependency_release_increments_both_pkgrel_values(self):
        payload = output(package("fomalhaut-web", "0.9.0-alpha.1"))
        matrix = resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR)
        self.assertEqual(
            [(entry["aur_package"], entry["pkgrel"]) for entry in matrix["include"]],
            [("greetd-fomalhaut", "4"), ("fomalhaut-lock", "3")],
        )

    def test_logind_release_increments_both_pkgrel_values(self):
        payload = output(package("fomalhaut-logind", "0.1.1-alpha.1"))
        matrix = resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR)
        self.assertEqual(
            [(entry["aur_package"], entry["pkgrel"]) for entry in matrix["include"]],
            [("greetd-fomalhaut", "4"), ("fomalhaut-lock", "3")],
        )

    def test_pam_release_only_rebuilds_locker(self):
        payload = output(package("fomalhaut-pam", "0.2.0-alpha.1"))
        matrix = resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR)
        self.assertEqual(
            [entry["aur_package"] for entry in matrix["include"]],
            ["fomalhaut-lock"],
        )

    def test_registry_recovery_plus_dependency_release_increments_pkgrel(self):
        payload = output(
            package(
                "fomalhaut",
                MANIFESTS["fomalhaut"],
                status="skipped",
                **{"skip-reason": "registry-version-exists"},
            ),
            package("fomalhaut-core", "0.8.0-alpha.1"),
        )
        matrix = resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR)
        self.assertEqual(matrix["include"][0]["pkgrel"], "4")

    def test_existing_registry_packages_do_not_trigger_dependency_rebuilds(self):
        payload = output(
            package(
                "fomalhaut",
                MANIFESTS["fomalhaut"],
                status="skipped",
                **{"skip-reason": "registry-version-exists"},
            ),
            package(
                "fomalhaut-lock",
                MANIFESTS["fomalhaut-lock"],
                status="skipped",
                **{"skip-reason": "registry-version-exists"},
            ),
            package(
                "fomalhaut-web",
                "0.8.0-alpha.1",
                status="skipped",
                **{"skip-reason": "registry-version-exists"},
            ),
            package("fomalhaut-sdk", "1.0.0-alpha.1"),
        )
        self.assertEqual(
            resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR),
            {"include": []},
        )

    def test_main_rerun_is_noop_when_aur_already_has_the_version(self):
        payload = output(package("fomalhaut", MANIFESTS["fomalhaut"]))
        self.assertEqual(
            resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR),
            {"include": []},
        )

    def test_irrelevant_publish_output_does_not_query_aur_packages(self):
        payload = output(package("fomalhaut-sdk", "1.0.0-alpha.1"))
        self.assertEqual(resolver.affected_aur_packages(payload), [])

    def test_dependency_release_cannot_create_a_missing_aur_package(self):
        payload = output(package("fomalhaut-pam", "0.2.0-alpha.1"))
        missing = dict(AUR)
        missing["fomalhaut-lock"] = None
        with self.assertRaises(resolver.ResolutionError):
            resolver.resolve_automatic(payload, SOURCE, MANIFESTS, missing)

    def test_rejects_unknown_schema_status_and_duplicate_package(self):
        invalid_payloads = [
            json.dumps({"schema-version": 2, "dry-run": False, "packages": []}),
            output(package("fomalhaut", MANIFESTS["fomalhaut"], status="unknown")),
            output(
                package("fomalhaut", MANIFESTS["fomalhaut"]),
                package("fomalhaut", MANIFESTS["fomalhaut"]),
            ),
        ]
        for payload in invalid_payloads:
            with self.subTest(payload=payload), self.assertRaises(resolver.ResolutionError):
                resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR)


class ManualResolutionTests(unittest.TestCase):
    def test_existing_version_requires_increasing_pkgrel(self):
        matrix = resolver.resolve_manual(
            "fomalhaut-lock", MANIFESTS["fomalhaut-lock"], SOURCE, 3, AUR["fomalhaut-lock"]
        )
        self.assertEqual(matrix["include"][0]["pkgrel"], "3")
        with self.assertRaises(resolver.ResolutionError):
            resolver.resolve_manual(
                "fomalhaut-lock",
                MANIFESTS["fomalhaut-lock"],
                SOURCE,
                2,
                AUR["fomalhaut-lock"],
            )


if __name__ == "__main__":
    unittest.main()
