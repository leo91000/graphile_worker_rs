# Graphile Worker upstream audit — 2026-09-18

## Inspection checkpoint

- Initial downstream main: `93045ddeb5fe45b81222312efb0255cd94d50028`.
- Official upstream main and published stable v0.18.0:
  `4cda192c5df254392a1dff350e5d73f7d2c18a85`.
- Stable maintenance branch 0.17.x and peeled v0.17.3:
  `ca2cef1c8c6bf9d9c818d19f194f596971c212df`.
- Incremental range: `4cda192c5df254392a1dff350e5d73f7d2c18a85..4cda192c5df254392a1dff350e5d73f7d2c18a85`
  (zero new commits). Live official refs, release metadata, CHANGELOG, tags and
  merged PRs were checked on September 18. No merged unreleased changes exist
  beyond the stable tag at this checkpoint.
- Reconstructed coverage retained from the [September 17 ledger](graphile-worker-js-upstream-2026-09-17.md):
  `5650fbc4406fa3ce197b2ab582e08fd20974e50c` (v0.16.6, exclusive) through
  `4cda192c5df254392a1dff350e5d73f7d2c18a85` (inclusive). A fresh first-parent
  enumeration found 105 commits and verified every SHA has an individual ledger
  entry. This is inspection coverage, not a claim of complete API parity.
- Existing [PR #524](https://github.com/leo91000/graphile_worker_rs/pull/524)
  was resumed instead of duplicating its ports. Its initial head was
  `409355727d3980ad4f8419424f91eed2b116800b`; none of those ports were present
  in initial main. Earlier [PR #514](https://github.com/leo91000/graphile_worker_rs/pull/514)
  is in main and its cron/parser/cleanup changes were checked in source.

## Revisited behavior and remaining work

The complete historical per-commit ledger remains in the September 17 report.
The following entries record this run's source comparison and revisit every
previously pending port; no unresolved entry is removed by advancing a date.

| Upstream | Disposition and evidence |
| --- | --- |
| [#622](https://github.com/graphile/worker/pull/622), `e9f3e3fd6160c883b7d659467608a2be8c3045cb` | PR #524 implements the same lock-then-deduplicate SQL as upstream `src/sql/getJobs.ts`, in Rust `batch_get_jobs.rs`. `tests/upstream_sync.rs` covers named queue serialization, unnamed jobs, priority, release, and concurrent claimers. |
| [#623](https://github.com/graphile/worker/pull/623), `be38db6319dc800f9f7a5b6be33dc9f499b2c586` | PR #524 adds migration 21 and filters existing task/queue identities before insertion, retaining conflict handling. Compared with upstream migration 20; Rust advisory key locks and GWNOR are preserved. Migration files 1–20 are unchanged. Upgrade regressions cover both revision-20 histories and exhausted sequences. |
| [#627](https://github.com/graphile/worker/pull/627), `9fc851b9f8087b309f5ff425b8d8f3e3497e3b45` | PR #524 caches fetch SQL by schema and query shape, with a per-thread 128-entry bound. Values remain parameters. Upstream JS helper closure hoisting and Record-to-Map conversion have no matching Rust allocation: Rust context shares owned handles. The previous database benchmark did not establish a speedup; no new performance claim is made. |
| [#557](https://github.com/graphile/worker/pull/557), `90ea8609593345dcd782b1728dfea97fcae09a21` | PR #524 contains observer panics in both future creation and polling, adapting upstream safeEmit to Rust futures. The regression also checks a surviving observer and repeated emission. Interceptor control flow and aborting panics remain documented differences. |
| [#474](https://github.com/graphile/worker/pull/474), `5a305b9424ddcee44ef38cc4aa9181bb756cc73b` | Existing batch/local-queue APIs remain; PR #524 adds SQLx persistent-statement opt-out to direct queries and wrapper-created transactions. Driver contract tests inspect server prepared statements. Retryable-failure batching and typed hooks retain their documented Rust contracts. |
| [#612](https://github.com/graphile/worker/pull/612), `7de3c85e004c5c6aafc0908c56c4db139e013758` | Rust already binds bigint arrays; PR #524 adds a regression with IDs above i32::MAX, checking failures and locks. |
| [#544](https://github.com/graphile/worker/pull/544), `b94bcf80ab496f63a0111107e7b8e052796b2f7f` | Already in main: capped indefinite cron recovery, registration/backfill before resumed scheduling and shutdown cancellation; compared upstream cron.ts with Rust runner/backfill and ADR 0001. Existing deterministic fault/recovery tests remain. |
| [#543](https://github.com/graphile/worker/pull/543), `bb03d1cd6bcabbab0e59c1db5c5c7651e46f2b5e` | Already in main: nested identifiers and strict parser tests preserve options, payload and following entries. |
| [#463](https://github.com/graphile/worker/pull/463) | Older cleanup gap remains fixed in main: null queue references are excluded from NOT IN and locked queues are retained; `worker_utils_cleanup/job_queues.rs` covers both. |
| [#502](https://github.com/graphile/worker/pull/502), `0ec5c7eb745635071ad8460466c5eb96d3fb2755` | Already in main: migration runner handles initialization clashes, preserves non-clash errors and tests concurrent migrators. |
| [#625](https://github.com/graphile/worker/pull/625), `9dcec769c8e86ddfe8b05af4c5289e5a18382339` | Implemented Test workflow matrix for PostgreSQL 14, 15, 16, 17 and 18, matching actual upstream CI. Existing test/doc-test commands and coverage jobs are retained; matrix concurrency is capped at two and fail-fast is disabled. Delivery and validation are recorded below. Node 22/24/26 jobs do not apply to compiled Rust. |

Open upstream [#633](https://github.com/graphile/worker/pull/633) (explicit
migration entrypoints) and [#634](https://github.com/graphile/worker/pull/634)
(Joi dependency) remain unmerged proposals, not changes to port this week.
ESM/type stripping/config discovery, TypeScript enums, ECMAScript disposal,
Jest and Node-specific packaging/CI changes retain their individual inapplicable
explanations in the historical ledger. Rust task registration, tracing,
WorkerOptions and explicit awaited shutdown remain intentional APIs.

## Validation and delivery

This report accompanies the recovery review fix in PR #524; exact-head CI and merge
results are recorded in the run checkpoint and final report. PostgreSQL 14–18
CI coverage does not raise Rust's PostgreSQL minimum or claim that all mixed
Node/Rust deployments were tested. No new full cross-language differential
or benchmark run is claimed.

## Recovery interaction found during review

The September 18 review approved the previous fixes but reported a valid
out-of-diff defect: replacing a locked keyed job exhausts its attempts, while
Rust recovery decremented them and revived the obsolete payload. A direct
PostgreSQL 18.6 reproduction on revision 21 returned both old and new jobs as
available (old attempts=24, max_attempts=25).

Migration 22 adds an explicit private superseded marker and preserves the
original lock ownership until release. All attempt-restoring paths keep marked
jobs exhausted, while ordinary final attempts still recover. Explicit SQL
rescheduling clears the marker. The proposed shortcut of clearing locks at
replacement was not used: fail_job/return_jobs require ownership and derive
queue unlocks from updated job rows, so that shortcut could strand a named queue.

Regression scenarios exercise dead-worker recovery, local-queue return, shutdown
return, direct/batched failure and successful completion, with and without named
queues, plus explicit rescheduling and normal final attempts. A revision-21
upgrade regression checks existing locked jobs remain unchanged and retryable.
Migrations 1–21 are unchanged. Historical replacements cannot be inferred from
old rows; the marker governs replacements after revision 22.

Pinned upstream `src/sql/returnJobs.ts` also decrements attempts without this
marker. This review fix is a downstream correction, not a claim of behavior
already fixed upstream. The new schema preserves row types and function
signatures, but Node v0.18.0/older Rust local-queue return queries do not acquire
the new protection automatically; the compatibility guide makes this explicit.

SQL-level differential check on PostgreSQL 18.6: installed all 20 official
upstream migrations at the pinned SHA in `upstream_fixture` and Rust migrations
1–22 in `rust_fixture`, both disposable schemas. Equivalent sequences of array
payload insertion, preserve_run_at replacement after an attempt, unsafe_dedupe,
and locked-key replacement produced identical public job projections. The
combined keyed-mode scenario ended with payload [1,2,3,4], run_at
2030-01-01T00:00:00Z, attempts=0 and revision=3; both task/queue identity sequences
remained 1. Locked replacement retained the exhausted original payload and owner,
and created an unlocked keyed [5] payload in both schemas. Runtime worker loops
were not involved. Separately, PostgreSQL 12.22 accepted all Rust migrations and
kept the superseded payload unavailable after dead-worker recovery.

The recovery review fix also covers remove_job and permanently_fail_jobs, which
retire jobs through the same exhausted-attempt state. A second controlled SQL
reproduction showed that a marker lookup inside one UPDATE still missed a
concurrently committed retirement after waiting for a row lock (attempts=24/25,
available=true). Return paths now share a private PL/pgSQL function that locks
jobs before reading markers in a fresh READ COMMITTED statement snapshot;
dead-worker recovery does the same. The bounded concurrency regression waits
for the database's actual Lock wait event before committing replacement.

The PostgreSQL 12 integration run exposed an existing test-helper limitation:
DROP DATABASE WITH (FORCE) is only supported from PostgreSQL 13. The helper now
uses version-aware cleanup, preventing new connections and terminating only the
UUID test database's sessions before dropping it on PostgreSQL 12. Assertions
and production code are unchanged by that test-tooling adaptation.

Explicit rescheduling uses the same lock-before-marker-read sequence. A controlled
concurrent permanently_fail_jobs/reschedule_jobs reproduction returned a revived
job with its retirement marker still present when both operations used a single
statement snapshot. The regression now verifies that rescheduling waits, clears
the committed marker, and permits ordinary attempt restoration on the next run.

Local PostgreSQL 12.22 validation of the final recovery SQL passed 22 focused
checks: 12 migration/upgrade tests, five retirement/recovery tests and five
upstream-sync tests. The explicit-reschedule regression also failed against
the earlier compiled migration (marker count 1 instead of 0), corroborating the
controlled SQL reproduction. Required pre-commit gates remain `just lint` and
`just test-docker`; exact results and final GitHub checks are recorded in the
persistent run checkpoint and delivery report. Builds use two jobs and tests
two threads to fit the container.

The PostgreSQL matrix change supersedes the historical September 17 #625
workflow-delivery deferral if delivered through an existing authorized GitHub
connection. It covers 14–18, not just the newly added 16–18 versions. Exact
delivery, review and CI status is retained in the persistent checkpoint.

The recovery implementation at `9248664c17e446b5ec221339c80b9c66038e0ae8`
passed `just lint`, `just test-docker` (437 passed, five ignored), mdBook,
and all three OpenTelemetry feature checks. The tokio-postgres focused run
passed 21 checks (11 migration, five retirement, five upstream scenarios).
The matrix change receives the same required local gates before commit and
all exact-head GitHub checks before merge.

A full PostgreSQL 12 run additionally found the same PostgreSQL-13-only DROP
DATABASE syntax in the separate CLI integration fixture. All of that test's
job-lifecycle assertions passed before cleanup failed with SQLSTATE 42601.
Its cleanup now also selects the supported syntax by server version and only
terminates connections to the UUID-named fixture. No lifecycle assertions or
production operations were removed or changed.

## Shutdown race exposed by CI

The combined-driver coverage job on `cddbaa6a785205e82cb59119fd6cd23d154b9c56`
failed the existing local_queue_returns_jobs_on_shutdown timeout
([CI evidence](https://github.com/leo91000/graphile_worker_rs/actions/runs/35343264278/job/105593845558)).
Source inspection found that an in-flight fetch could append jobs after release
had drained the cache and change Released back to Waiting. A concurrent release
also returned before the first caller completed cleanup. A controlled regression
pauses the fetch-completion hook, begins two release calls, then resumes the
fetch. It failed against the earlier source because the second release returned
early. The corrected release serializes callers, preserves Released as a terminal
state, uses persistent run-completion state, and drains only after fetching ends.
A stored wake-up permit also closes the mode-check/wait registration race.
Failed return attempts preserve cached jobs for a later release retry.

All 19 local-queue tests passed after this correction, including the original
shutdown timeout and the new controlled concurrency regression. The existing
timeout and queue-return assertions remain unchanged. This is a downstream
shutdown correction exposed while validating the upstream ports, not a claim
that upstream contains the same defect or implementation.

During validation, downstream main advanced to
`b6b8f470cd387ac62edbcfc1ea2eb34f0f883808` through the Lucide icon dependency
update in PR #525. That main revision is incorporated before final validation;
the initial audit baseline remains `93045ddeb5fe45b81222312efb0255cd94d50028`.

## Documentation review follow-up

The final functional review covered `62067076f1d404b09438374e8f5963fc7c7d98e0`
with no actionable correctness findings. Its aggregate function-documentation
check warned about missing Rustdoc, so affected queue, recovery, registration,
SQLx and observer contracts now have function documentation, alongside the
failure scenarios guarded by the regression tests. This follow-up changes
comments only and receives the required validation and another review.

The review bot's description check is inapplicable: the repository owner
explicitly requires an empty PR body. Intent, implementation and validation
remain in these committed reports, the ADR, review replies and final delivery
report. The PR description is not populated to satisfy a conflicting suggestion.
