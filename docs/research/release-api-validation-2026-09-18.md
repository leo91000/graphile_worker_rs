# Release API validation repair — 2026-09-18

## Observed failure and scope

The upstream audit found that main at
`b6b8f470cd387ac62edbcfc1ea2eb34f0f883808` already failed its automatic
[Release-plz run](https://github.com/leo91000/graphile_worker_rs/actions/runs/35343682260).
Standalone database-crate documentation could not compile `tokio::select!`
because the dependency omitted the `macros` feature. The current manifest now
explicitly enables `macros` and `time`, matching the code's actual requirements.
This is also included in the independent upstream-port PR #524.

The published `graphile_worker_database` 0.1.6 baseline has the same missing
features. Enabling namespaced dependency features through the checker's
`--baseline-features tokio/macros,tokio/time` did not repair its build. A native
cargo-semver-checks 0.50.0 experiment using a disposable copy of that published
package, changing only its Tokio manifest features, passed 196 checks (58
inapplicable checks skipped by the native checker). All published Rust source
and license files remained unchanged. Published packages are immutable; this
repair does not republish or alter them.

Several workspace crates also expose mutually exclusive OpenTelemetry 0.30,
0.31 and 0.32 integrations. The native checker's default selection enables all
stable features together, triggering the intentional compile-time guard. The
repair retains that guard and checks each supported version separately, plus
no telemetry. Every other stable feature remains selected in every profile,
including both database drivers and the existing default feature group. The
union of profiles covers every feature selected by the native checker's 0.50
stable-feature policy. Both sides use their own declared features, so a version
present on only one side is still compared.

## Implementation and failure policy

`.github/scripts/semver_checks.py` wraps only release-plz's known package-specific
`cargo semver-checks check-release` invocation. The real installed checker stays
authoritative. Packages without telemetry retain its native feature selection.
All original release policy and other arguments are preserved. API
incompatibility from any profile remains exit 100 even if another profile
succeeds; compilation and tool errors remain fatal. Unexpected feature overrides
fail visibly rather than silently changing API coverage.

Only the known published database package version 0.1.6 receives a disposable
baseline manifest copy with the missing Tokio features added and an empty
workspace table to preserve standalone package resolution inside the build
directory. The real-checker preflight reproduced Cargo's parent-workspace error
before this boundary was added. Rust source and
license bytes are preserved; the original baseline is untouched. Copies live
under the build target directory and are cleaned after the checker exits.
The same database build-feature fix applies when 0.1.6 is a transitive baseline
dependency. The real queries-0.1.2 preflight reproduced the missing
`tokio::select!` error in that dependency before this extension. Cargo metadata identifies the exact published package; a temporary
Cargo configuration patches only that dependency to the repaired published copy.
A configuration-level patch is necessary because the checker builds a generated
parent package, which ignores dependency-manifest patches. The checked package
manifest and all dependency Rust source remain unchanged. This follows
[Cargo's documented configuration patch mechanism](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section).
Unknown manifest layouts fail. This exception can be removed after release-plz
no longer compares against that published baseline.

The release workflow expands its existing composite action into explicit steps
so the adapter can be installed after the real checker. Tool versions and action
commit pins match the resolved `MarcoIeni/release-plz-action@v0.5` implementation:
cargo-semver-checks 0.50, cargo-binstall 1.23.0, and release-plz 0.3.168. The existing
automatic release-pr followed by release order, tokens, push-to-main trigger and
permissions are retained. A shared Cargo target directory reuses dependency
builds across API profiles; disabled debug symbols bound cache size without
changing assertions or API checks. The audit runs no manual release or tag command.

## Validation

Regression tests exercise profile coverage, driver preservation, paths with
spaces, release-policy arguments, incompatibility/error propagation, baseline
version scoping, untouched Rust/license bytes, temporary-copy cleanup, unknown
layout rejection and version-probe delegation. The existing Check workflow runs
these tests before its unchanged Rust checks. Required repository validation is
`just lint` and `just test-docker`; the adapter is additionally exercised against
the real pinned checker before delivery. A real-checker fixture detected a
public API removed only under OpenTelemetry 0.31 (exit 100), continued through
the remaining profile, and passed all four profiles after restoring that API
(exit 0). Exact commands, outcomes and delivery
SHAs are recorded in the durable audit checkpoint and final run report.

This repair is separate from queue behavior so either change can be reviewed
independently. It disables no API check, changes no published baseline Rust API,
and does not claim full upstream behavioral parity.

## Completed local API preflight

The final adapter passed the real cargo-semver-checks 0.50.0 comparison against
published baselines for database 0.1.6, queries 0.1.2 and worker 0.13.5. Database
used native feature selection; queries and worker each passed no-telemetry and
OpenTelemetry 0.30/0.31/0.32 profiles. Each invocation reported 196 checks passed
and 58 native skips, with no version bump required. The negative fixture still
returned 100 for an API removed only in the 0.31 profile, then returned zero
after that API was restored. Fourteen adapter regression tests pass.

The related upstream-port update `ad8456e62ee8516a2bb65372b8e60c0698343712`
passed required lint and full PostgreSQL 18 and 12 suites (447 tests, five
ignored in each), plus 24 async-std local-queue/recovery integration tests.
Its exact-head PostgreSQL 14–18 matrix, all three coverage variants, Check,
documentation build and Codecov checks passed before re-review was requested.
Review approval and main CI remain separate delivery gates, recorded in the
final audit report and durable run checkpoint.

## Review follow-up

The adapter regression now uses distinct current and baseline feature sets,
including telemetry versions and driver features unique to each side. It asserts
each profile label and both exact feature arguments, so copying or swapping the
feature lists fails the test.

The Check and Release-plz workflows pin their previously mutable checkout,
Rust-toolchain and cache actions to the commits resolved from their existing
refs on September 18. Existing action inputs and toolchain selection are
preserved. Release checkout disables credential persistence; release-plz still
receives its token explicitly. This workflow does not sign tags or run a separate
git push, matching the [documented API-based authentication contract](https://release-plz.dev/docs/github/persist-credentials).
