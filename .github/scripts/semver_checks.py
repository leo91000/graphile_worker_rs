#!/usr/bin/env python3
"""Adapt release-plz's API checks to this workspace's supported build configurations.

The real checker remains authoritative: failures and incompatibilities propagate.
Published Rust source is never changed, and every supported telemetry version is
checked separately instead of enabling mutually exclusive versions together.
"""
from __future__ import annotations

from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

TELEMETRY = {f"opentelemetry_0_{version}" for version in (30, 31, 32)}
FEATURE_FLAGS = {
    "--features", "--baseline-features", "--current-features", "--all-features",
    "--default-features", "--only-explicit-features",
}


def option(args: list[str], name: str) -> str | None:
    """Read one long option without interpreting shell syntax or dropping arguments."""
    found = []
    for index, value in enumerate(args):
        if value == name:
            if index + 1 == len(args):
                raise ValueError(f"missing value for {name}")
            found.append(args[index + 1])
        elif value.startswith(name + "="):
            found.append(value.split("=", 1)[1])
    if len(found) > 1:
        raise ValueError(f"duplicate {name} is not supported by the release adapter")
    return found[0] if found else None


def replace_option(args: list[str], name: str, value: str) -> list[str]:
    """Replace only the requested option, preserving every other checker argument."""
    result = list(args)
    for index, arg in enumerate(result):
        if arg == name:
            result[index + 1] = value
            return result
        if arg.startswith(name + "="):
            result[index] = name + "=" + value
            return result
    raise ValueError(f"missing required {name}")


def metadata(manifest: Path, package: str) -> dict:
    """Ask Cargo for the exact package features, including implicit optional dependencies."""
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1",
         "--manifest-path", str(manifest)],
        check=True, capture_output=True, text=True,
    )
    matches = [item for item in json.loads(result.stdout)["packages"]
               if item["name"] == package]
    if len(matches) != 1:
        raise ValueError(f"expected one package named {package} in {manifest}")
    return matches[0]


def stable_features(features: dict) -> set[str]:
    """Retain the real checker's default stable-feature heuristic (version 0.50)."""
    return {name for name in features
            if name not in {"unstable", "nightly", "bench", "no_std"}
            and not name.startswith(("_", "unstable-", "unstable_"))}


def profiles(current: dict, baseline: dict) -> list[tuple[str, set[str], set[str]]]:
    """Cover no telemetry and every telemetry version present on either side."""
    current_features = stable_features(current["features"])
    baseline_features = stable_features(baseline["features"])
    versions = (current_features | baseline_features) & TELEMETRY
    if not versions:
        return []
    result = []
    for version in [None, *sorted(versions)]:
        current_set = current_features - TELEMETRY
        baseline_set = baseline_features - TELEMETRY
        if version in current_features:
            current_set.add(version)
        if version in baseline_features:
            baseline_set.add(version)
        result.append((version or "without telemetry", current_set, baseline_set))
    return result


@contextmanager
def buildable_baseline(manifest: Path, scratch: Path):
    """Repair only missing Tokio build features in a disposable published-0.1.6 copy."""
    source = manifest.read_text()
    data = tomllib.loads(source)
    package = data.get("package", {})
    if (package.get("name"), package.get("version")) != (
        "graphile_worker_database", "0.1.6"
    ):
        yield manifest
        return
    features = data["dependencies"]["tokio"].get("features", [])
    missing = sorted({"macros", "time"} - set(features))
    if not missing:
        yield manifest
        return
    section = re.search(r"(?ms)^\[dependencies\.tokio\]\s*\n(.*?)(?=^\[|\Z)", source)
    if not section:
        raise ValueError("published database baseline has no normalized Tokio dependency")
    feature_list = re.search(r"(?ms)^features\s*=\s*\[[^\]]*\]", section[1])
    if not feature_list:
        raise ValueError("published database baseline has no Tokio feature array")
    start = section.start(1) + feature_list.start()
    end = section.start(1) + feature_list.end()
    repaired = source[:start] + "features = " + json.dumps(features + missing) + source[end:]
    # A copy beneath this repository's target directory must not inherit its
    # parent workspace; the published package is checked as a standalone crate.
    if "workspace" in package:
        raise ValueError("published database baseline unexpectedly inherits a workspace")
    if "workspace" not in data:
        repaired += "\n[workspace]\n"
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="database-0.1.6-", dir=scratch) as directory:
        root = Path(directory) / "baseline"
        shutil.copytree(manifest.parent, root, ignore=shutil.ignore_patterns("target", ".git"))
        destination = root / manifest.name
        destination.write_text(repaired)
        print("API baseline: enabling missing Tokio macros/time in a temporary 0.1.6 "
              "manifest; published Rust source is unchanged", file=sys.stderr)
        yield destination



def published_database_dependency(manifest: Path) -> Path | None:
    """Locate only the known published database dependency in a project baseline."""
    package = tomllib.loads(manifest.read_text()).get("package", {})
    name = package.get("name", "")
    if not name.startswith("graphile_worker") or name == "graphile_worker_database":
        return None
    result = subprocess.run(
        ["cargo", "metadata", "--all-features", "--format-version", "1",
         "--manifest-path", str(manifest)],
        check=True, capture_output=True, text=True,
    )
    matches = [item for item in json.loads(result.stdout)["packages"]
               if item["name"] == "graphile_worker_database" and item["version"] == "0.1.6"
               and item.get("source") == "registry+https://github.com/rust-lang/crates.io-index"]
    if len(matches) > 1:
        raise ValueError("ambiguous published database baseline dependency")
    return Path(matches[0]["manifest_path"]) if matches else None


@contextmanager
def baseline_environment(manifest: Path, scratch: Path):
    """Apply the same manifest-only repair when 0.1.6 is a transitive dependency."""
    dependency = published_database_dependency(manifest)
    if dependency is None:
        yield None
        return
    with buildable_baseline(dependency, scratch) as repaired:
        if repaired == dependency:
            yield None
            return
        with tempfile.TemporaryDirectory(prefix="api-environment-", dir=scratch) as directory:
            cwd = Path(directory)
            (cwd / ".cargo").mkdir()
            (cwd / ".cargo/config.toml").write_text(
                "[patch.crates-io.graphile_worker_database]\npath = "
                + json.dumps(str(repaired.parent)) + "\n"
            )
            # A manifest-level patch on the baseline would be ignored by the
            # checker's generated parent package. Cargo config applies to that
            # parent as well, while leaving both checked package manifests alone.
            print("API baseline: applying the 0.1.6 build-feature repair transitively",
                  file=sys.stderr)
            yield cwd


def run_checks(commands: list[tuple[str, list[str]]], run=subprocess.run,
               cwd: Path | None = None) -> int:
    """Propagate all API incompatibilities and stop on an actual checker/build error."""
    outcome = 0
    for label, command in commands:
        print(f"API profile: {label}", file=sys.stderr)
        result = run(command, check=False, **({"cwd": cwd} if cwd is not None else {}))
        if result.returncode == 100:
            outcome = 100
        elif result.returncode != 0:
            return result.returncode if result.returncode > 0 else 128 - result.returncode
    return outcome


def main(args: list[str]) -> int:
    """Wrap release-plz's known invocation and delegate informational commands unchanged."""
    real = os.environ.get("GRAPHILE_SEMVER_CHECKS_BIN")
    if not real:
        raise ValueError("GRAPHILE_SEMVER_CHECKS_BIN must identify the installed real checker")
    real_path = Path(real).resolve()
    wrapper = Path(__file__).parent / "semver-bin" / "cargo-semver-checks"
    if real_path == wrapper.resolve() or real_path == Path(__file__).resolve():
        raise ValueError("the real checker must not be the adapter")
    if args[:2] != ["semver-checks", "check-release"]:
        return subprocess.run([str(real_path), *args], check=False).returncode
    if any(arg.split("=", 1)[0] in FEATURE_FLAGS for arg in args):
        raise ValueError("release-plz supplied feature overrides; review the adapter configuration")
    manifest_arg = option(args, "--manifest-path")
    baseline_arg = option(args, "--baseline-root")
    package = option(args, "--package")
    if not all((manifest_arg, baseline_arg, package)):
        raise ValueError("release-plz must supply manifest, baseline and one explicit package")
    manifest = Path(manifest_arg).resolve()
    baseline = Path(baseline_arg).resolve()
    if baseline.is_dir():
        baseline /= "Cargo.toml"
    scratch = Path(os.environ.get("GRAPHILE_SEMVER_SCRATCH_ROOT", "target/release-api-baselines"))
    scratch = scratch.resolve()
    with buildable_baseline(baseline, scratch) as checked_baseline, \
            baseline_environment(checked_baseline, scratch) as cwd:
        checked_args = replace_option(args, "--baseline-root", str(checked_baseline))
        checked_args = replace_option(checked_args, "--manifest-path", str(manifest))
        current = metadata(manifest, package)
        previous = metadata(checked_baseline, package)
        variants = profiles(current, previous)
        if not variants:
            return run_checks([("native stable-feature selection", [str(real_path), *checked_args])], cwd=cwd)
        commands = []
        for label, current_features, baseline_features in variants:
            command = [str(real_path), *checked_args, "--only-explicit-features"]
            if current_features:
                command.extend(["--current-features", ",".join(sorted(current_features))])
            if baseline_features:
                command.extend(["--baseline-features", ",".join(sorted(baseline_features))])
            commands.append((label, command))
        return run_checks(commands, cwd=cwd)


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr, end="")
        print(f"API check adapter failed: {error}", file=sys.stderr)
        sys.exit(101)
