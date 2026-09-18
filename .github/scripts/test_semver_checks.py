"""Regression tests for preserving API coverage and failure signals in release checks."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("semver_checks", Path(__file__).with_name("semver_checks.py"))
adapter = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(adapter)


class ApiCheckTests(unittest.TestCase):
    def test_profiles_cover_each_supported_version_and_other_stable_features(self):
        """Every supported telemetry API and every other stable feature must be checked."""
        current = {"features": {name: [] for name in (
            "default", "driver-sqlx", "driver-tokio-postgres", "opentelemetry_0_30",
            "opentelemetry_0_31", "opentelemetry_0_32", "_private", "nightly",
        )}}
        baseline = {"features": {name: [] for name in (
            "default", "driver-sqlx", "opentelemetry_0_30", "opentelemetry_0_31",
        )}}
        profiles = adapter.profiles(current, baseline)
        self.assertEqual(len(profiles), 4)
        for _, current_set, baseline_set in profiles:
            self.assertLessEqual(len(current_set & adapter.TELEMETRY), 1)
            self.assertLessEqual(len(baseline_set & adapter.TELEMETRY), 1)
            self.assertTrue({"default", "driver-sqlx", "driver-tokio-postgres"} <= current_set)
            self.assertTrue({"default", "driver-sqlx"} <= baseline_set)
        self.assertEqual(set.union(*(item[1] for item in profiles)), adapter.stable_features(current["features"]))
        self.assertEqual(set.union(*(item[2] for item in profiles)), adapter.stable_features(baseline["features"]))
        self.assertEqual(profiles[-1][2] & adapter.TELEMETRY, set())

    def test_packages_without_telemetry_keep_native_feature_selection(self):
        """Ordinary crates keep the native checker feature policy."""
        self.assertEqual(adapter.profiles({"features": {"default": []}}, {"features": {}}), [])

    def test_option_replacement_preserves_paths_and_other_checker_flags(self):
        """Paths with spaces and release policy flags survive argument adaptation."""
        arguments = ["--release-type", "minor", "--baseline-root=/a path/Cargo.toml", "--package", "worker"]
        result = adapter.replace_option(arguments, "--baseline-root", "/another path/Cargo.toml")
        self.assertEqual(adapter.option(result, "--baseline-root"), "/another path/Cargo.toml")
        self.assertEqual(result[:2], arguments[:2])
        self.assertEqual(result[-2:], arguments[-2:])
        with self.assertRaises(ValueError):
            adapter.option(["--package", "one", "--package", "two"], "--package")

    def test_incompatibility_from_any_profile_is_not_hidden_by_later_success(self):
        """A later successful profile must not erase an earlier API incompatibility."""
        calls = []
        def checker(command, check):
            calls.append(command)
            return subprocess.CompletedProcess(command, [0, 100, 0][len(calls) - 1])
        self.assertEqual(adapter.run_checks([(str(i), [str(i)]) for i in range(3)], checker), 100)
        self.assertEqual(len(calls), 3)

    def test_checker_build_failure_is_never_treated_as_an_api_result(self):
        """A build error must remain a fatal error, not an API compatibility result."""
        calls = []
        def checker(command, check):
            calls.append(command)
            return subprocess.CompletedProcess(command, 101)
        self.assertEqual(adapter.run_checks([("first", ["one"]), ("second", ["two"])], checker), 101)
        self.assertEqual(calls, [["one"]])

    def test_all_native_checks_must_succeed(self):
        """Only success from every executed profile produces overall success."""
        self.assertEqual(adapter.run_checks([("one", ["one"]), ("two", ["two"])],
                         lambda command, check: subprocess.CompletedProcess(command, 0)), 0)

    def test_baseline_fix_changes_only_known_missing_build_features(self):
        """Only known build features change; source, license and original baseline stay intact."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "published"
            (source / "src").mkdir(parents=True)
            manifest = source / "Cargo.toml"
            original = ('[package]\nname = "graphile_worker_database"\nversion = "0.1.6"\n'
                        '[dependencies.tokio]\nversion = "1.53.0"\nfeatures = ["rt", "sync"]\noptional = true\n')
            manifest.write_text(original)
            rust = b"pub fn existing_api() {}\n"
            (source / "src/lib.rs").write_bytes(rust)
            (source / "LICENSE").write_text("retained license fixture")
            with adapter.buildable_baseline(manifest, root / "scratch") as copied:
                self.assertNotEqual(copied, manifest)
                repaired = adapter.tomllib.loads(copied.read_text())
                self.assertEqual(repaired["dependencies"]["tokio"]["features"], ["rt", "sync", "macros", "time"])
                self.assertEqual(repaired["workspace"], {})
                self.assertEqual((copied.parent / "src/lib.rs").read_bytes(), rust)
                self.assertEqual((copied.parent / "LICENSE").read_text(), "retained license fixture")
                self.assertEqual(manifest.read_text(), original)
            self.assertFalse(copied.exists())
            self.assertEqual(manifest.read_text(), original)
            manifest.write_text(original.replace('version = "0.1.6"', 'version = "0.1.7"'))
            with adapter.buildable_baseline(manifest, root / "scratch") as unchanged:
                self.assertEqual(unchanged, manifest)

    def test_changed_baseline_layout_fails_instead_of_silently_skipping_checks(self):
        """Unknown published manifest layouts must fail visibly."""
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text('[package]\nname="graphile_worker_database"\nversion="0.1.6"\n'
                                '[dependencies.tokio]\nversion="1.53"\n')
            with self.assertRaises(ValueError), adapter.buildable_baseline(manifest, Path(directory) / "scratch"):
                self.fail("an unknown manifest shape must fail closed")

    def test_main_invokes_real_checker_for_each_profile_with_original_policy(self):
        """Every generated command retains the real checker and the original release policy."""
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text('[package]\nname="worker"\nversion="1.0.0"\n')
            args = ["semver-checks", "check-release", "--release-type", "minor", "--color", "never",
                    "--manifest-path", str(manifest), "--package", "worker", "--baseline-root", str(manifest)]
            calls = []
            def checks(commands, **kwargs):
                calls.extend(commands)
                return 100
            features = {"features": {"default": [], "opentelemetry_0_30": [], "opentelemetry_0_31": []}}
            with patch.dict(os.environ, {"GRAPHILE_SEMVER_CHECKS_BIN": "/real/checker"}), \
                    patch.object(adapter, "metadata", return_value=features), \
                    patch.object(adapter, "run_checks", side_effect=checks):
                self.assertEqual(adapter.main(args), 100)
            self.assertEqual(len(calls), 3)
            for _, command in calls:
                self.assertEqual(command[0], "/real/checker")
                self.assertEqual(adapter.option(command, "--release-type"), "minor")
                self.assertIn("--only-explicit-features", command)
                self.assertIn("--current-features", command)
                self.assertIn("--baseline-features", command)

    def test_unexpected_feature_overrides_fail_instead_of_reducing_coverage(self):
        """Changed caller feature policy requires an explicit adapter update."""
        with patch.dict(os.environ, {"GRAPHILE_SEMVER_CHECKS_BIN": "/real/checker"}):
            with self.assertRaises(ValueError):
                adapter.main(["semver-checks", "check-release", "--default-features"])

    def test_transitive_repair_uses_only_published_source_and_temporary_config(self):
        """A transitive repair must preserve the checked manifest and all Rust bytes."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            database = root / "published-database"
            (database / "src").mkdir(parents=True)
            manifest = database / "Cargo.toml"
            original = ('[package]\nname="graphile_worker_database"\nversion="0.1.6"\n'
                        '[dependencies.tokio]\nversion="1.53"\nfeatures=["rt","sync"]\n')
            manifest.write_text(original)
            (database / "src/lib.rs").write_text("pub fn published_api() {}\n")
            checked = root / "Cargo.toml"
            checked_source = '[package]\nname="graphile_worker_queries"\nversion="0.1.2"\n'
            checked.write_text(checked_source)
            with patch.object(adapter, "published_database_dependency", return_value=manifest):
                with adapter.baseline_environment(checked, root / "scratch") as cwd:
                    config = adapter.tomllib.loads((cwd / ".cargo/config.toml").read_text())
                    copied = Path(config["patch"]["crates-io"]["graphile_worker_database"]["path"])
                    self.assertEqual((copied / "src/lib.rs").read_bytes(), (database / "src/lib.rs").read_bytes())
                    self.assertEqual(checked.read_text(), checked_source)
                    self.assertEqual(manifest.read_text(), original)
                self.assertFalse(cwd.exists())
                self.assertFalse(copied.exists())

    def test_transitive_lookup_is_scoped_to_the_known_crates_io_version(self):
        """Other releases and other registries must never receive the baseline patch."""
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text('[package]\nname="graphile_worker_queries"\nversion="0.1.2"\n')
            known = {"name": "graphile_worker_database", "version": "0.1.6",
                     "source": "registry+https://github.com/rust-lang/crates.io-index",
                     "manifest_path": "/published/database/Cargo.toml"}
            others = [{**known, "version": "0.1.7"}, {**known, "source": "registry+https://elsewhere.invalid"}]
            result = subprocess.CompletedProcess([], 0, stdout=json.dumps({"packages": others}))
            with patch.object(adapter.subprocess, "run", return_value=result):
                self.assertIsNone(adapter.published_database_dependency(manifest))
            result.stdout = json.dumps({"packages": [known, *others]})
            with patch.object(adapter.subprocess, "run", return_value=result):
                self.assertEqual(adapter.published_database_dependency(manifest), Path(known["manifest_path"]))

    def test_temporary_config_directory_reaches_every_checker_profile(self):
        """The generated Cargo configuration must cover all real checker subprocesses."""
        calls = []
        def checker(command, **kwargs):
            calls.append(kwargs)
            return subprocess.CompletedProcess(command, 0)
        self.assertEqual(adapter.run_checks([("one", ["one"]), ("two", ["two"])],
                                           checker, cwd=Path("/temporary/config")), 0)
        self.assertEqual(calls, [{"check": False, "cwd": Path("/temporary/config")}] * 2)

    def test_version_probe_delegates_to_installed_checker(self):
        """Tool installation probes must reach the installed checker unchanged."""
        with patch.dict(os.environ, {"GRAPHILE_SEMVER_CHECKS_BIN": "/real/checker"}), \
                patch.object(adapter.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as run:
            self.assertEqual(adapter.main(["--version"]), 0)
            run.assert_called_once_with(["/real/checker", "--version"], check=False)


if __name__ == "__main__":
    unittest.main()
