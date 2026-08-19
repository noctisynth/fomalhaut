import importlib.util
import json
import sys
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("resolve.py")
REPOSITORY = MODULE_PATH.parents[2]
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("fomalhaut_aur_resolve", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("test could not load AUR resolver")
resolver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(resolver)

SOURCE = "a" * 40
THEME = "@fomalhaut/theme-nocturne"
MANIFESTS = {
    "fomalhaut": "1.2.0-alpha.1",
    "fomalhaut-lock": "0.3.0-alpha.2",
    THEME: "0.0.1",
}
AUR = {
    "greetd-fomalhaut": "1.2.0.alpha.1-3",
    "fomalhaut-lock": "0.3.0.alpha.2-2",
    "fomalhaut-theme-nocturne": "0.0.1-1",
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

    def test_user_integration_release_increments_both_pkgrel_values(self):
        payload = output(package("fomalhaut-user", "0.1.1-alpha.1"))
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

    def test_private_nocturne_release_synchronizes_aur_version(self):
        payload = output(
            package(
                THEME,
                "0.0.2",
                status="skipped",
                **{"skip-reason": "private"},
            )
        )
        manifests = dict(MANIFESTS)
        manifests[THEME] = "0.0.2"
        matrix = resolver.resolve_automatic(payload, SOURCE, manifests, AUR)
        self.assertEqual(
            matrix,
            {
                "include": [
                    {
                        "aur_package": "fomalhaut-theme-nocturne",
                        "upstream_version": "0.0.2",
                        "pkgver": "0.0.2",
                        "pkgrel": "1",
                        "source_ref": SOURCE,
                    }
                ]
            },
        )

    def test_private_nocturne_rerun_is_noop(self):
        payload = output(
            package(
                THEME,
                MANIFESTS[THEME],
                status="skipped",
                **{"skip-reason": "private"},
            )
        )
        self.assertEqual(
            resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR),
            {"include": []},
        )

    def test_only_exact_private_nocturne_result_is_affected(self):
        private_records = [
            package(
                THEME,
                MANIFESTS[THEME],
                status="skipped",
                **{"skip-reason": "missing-changelog"},
            ),
            package(
                "fomalhaut-sdk",
                "1.0.0-alpha.1",
                status="skipped",
                **{"skip-reason": "private"},
            ),
            package(THEME, MANIFESTS[THEME]),
        ]
        for record in private_records:
            with self.subTest(record=record):
                self.assertEqual(resolver.affected_aur_packages(output(record)), [])

    def test_private_nocturne_version_must_match_manifest(self):
        payload = output(
            package(
                THEME,
                "0.0.2",
                status="skipped",
                **{"skip-reason": "private"},
            )
        )
        with self.assertRaises(resolver.ResolutionError):
            resolver.resolve_automatic(payload, SOURCE, MANIFESTS, AUR)

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

    def test_nocturne_accepts_a_new_upstream_version(self):
        matrix = resolver.resolve_manual(
            "fomalhaut-theme-nocturne",
            "0.0.2",
            SOURCE,
            1,
            AUR["fomalhaut-theme-nocturne"],
        )
        self.assertEqual(matrix["include"][0]["pkgver"], "0.0.2")


class NodeManifestTests(unittest.TestCase):
    def test_requires_exact_private_package(self):
        manifest = json.dumps({"name": THEME, "private": True, "version": "0.0.2"})
        self.assertEqual(resolver.node_manifest_version(manifest, THEME), "0.0.2")
        invalid = [
            json.dumps({"name": "other", "private": True, "version": "0.0.2"}),
            json.dumps({"name": THEME, "private": False, "version": "0.0.2"}),
            json.dumps({"name": THEME, "private": True, "version": "next"}),
            "[]",
        ]
        for content in invalid:
            with self.subTest(content=content), self.assertRaises(resolver.ResolutionError):
                resolver.node_manifest_version(content, THEME)


class AurNpmManifestTests(unittest.TestCase):
    def test_build_manifest_mirrors_theme_and_sdk_dependencies(self):
        theme = json.loads(
            (REPOSITORY / "themes/nocturne/package.json").read_text(encoding="utf-8")
        )
        sdk = json.loads(
            (REPOSITORY / "packages/fomalhaut-sdk/package.json").read_text(
                encoding="utf-8"
            )
        )
        build_directory = (
            REPOSITORY / "packaging/aur/fomalhaut-theme-nocturne"
        )
        build = json.loads(
            (build_directory / "package.json").read_text(encoding="utf-8")
        )
        lock = json.loads(
            (build_directory / "package-lock.json").read_text(encoding="utf-8")
        )

        expected_dependencies = dict(theme["dependencies"])
        self.assertEqual(expected_dependencies.pop("fomalhaut-sdk"), "workspace:*")
        expected_dev_dependencies = dict(sdk["devDependencies"])
        expected_dev_dependencies.update(theme["devDependencies"])

        self.assertEqual(build["dependencies"], expected_dependencies)
        self.assertEqual(build["devDependencies"], expected_dev_dependencies)
        self.assertTrue(build["private"])
        self.assertEqual(build["license"], "0BSD")
        self.assertEqual(lock["lockfileVersion"], 3)
        self.assertEqual(lock["packages"][""]["license"], "0BSD")
        self.assertEqual(lock["packages"][""]["dependencies"], expected_dependencies)
        self.assertEqual(
            lock["packages"][""]["devDependencies"], expected_dev_dependencies
        )


if __name__ == "__main__":
    unittest.main()
