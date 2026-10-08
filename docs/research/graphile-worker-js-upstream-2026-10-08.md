# Graphile Worker upstream audit — 2026-10-08

## Result and inspection checkpoint

This is a **behavioral no-op**: no new merged upstream behavior or still-missing
applicable runtime port was found in the reconstructed audit range. This documentation
update records current source evidence, closes the historical CI-delivery
deferral and clarifies existing cron clock compatibility; it does not claim
full JavaScript API or runtime parity.

- Downstream baseline: `37075d856efec202c637f4052be710cb92cd5be3` on
  `leo91000/graphile_worker_rs/main`.
- Official upstream main and peeled stable `v0.18.0`:
  `4cda192c5df254392a1dff350e5d73f7d2c18a85`.
- Stable maintenance branch `0.17.x` and peeled `v0.17.3`:
  `ca2cef1c8c6bf9d9c818d19f194f596971c212df`.
- Incremental inspected range:
  `4cda192c5df254392a1dff350e5d73f7d2c18a85..4cda192c5df254392a1dff350e5d73f7d2c18a85`
  (zero commits).
- Retained historical coverage:
  `5650fbc4406fa3ce197b2ab582e08fd20974e50c` (peeled `v0.16.6`, exclusive)
  through `4cda192c5df254392a1dff350e5d73f7d2c18a85` (inclusive).
  A fresh first-parent enumeration found 105 commits, each represented by an
  exact commit link in the [September 17 ledger](graphile-worker-js-upstream-2026-09-17.md).
  First-parent comparisons include the full merge delta. The checkpoint remains
  separate from implementation coverage and the limits below.

Official live branches and tags were fetched from `graphile/worker` into a
read-only comparison checkout. The audit inspected current CHANGELOG, release
metadata, recent merged PRs, source diffs and relevant source/tests. Published
stable behavior is v0.18.0 (published September 8); **merged but unreleased
changes: none** beyond that tag. Release objects were not the sole signal.

Durable state was absent in this run's environment. The checkpoint was recovered
from committed September 17/18 reports, verified against live upstream history,
and checked against downstream merge history. The missing September 5 report
still does not establish coverage for unknown older gaps.

## Delivery verification and revisited ledger

[PR #514](https://github.com/leo91000/graphile_worker_rs/pull/514), merge
`fd60d5a210aa758830471de156ddce3d9ae2b881`, and
[PR #524](https://github.com/leo91000/graphile_worker_rs/pull/524), merge
`4d2e979eb50fb292b9ab10eaf101edfca2546d13`, are ancestors of the baseline.
PR #524's final head `2955bc8ae0e4f0c45d6400409bda1945bef55c6c` received an
actual APPROVED review and successful Check, Test, Coverage, Docs and coverage
statuses. Current source was inspected independently of those PR titles.
There is no open upstream-port PR to duplicate. Unrelated dependency and release
PRs are outside this audit's delivery scope.

All 105 historical per-commit dispositions remain in the September 17 ledger;
the following entries supersede its proposed-port and deferred-delivery statuses
with evidence from the current baseline. They also revisit each previously
pending behavior. No unresolved entry is discarded by retaining the inspected SHA.

| Upstream commit / PR | Behavior and affected Rust components | Current disposition and source/test evidence |
| --- | --- | --- |
| [e9f3e3f / #622](https://github.com/graphile/worker/commit/e9f3e3fd6160c883b7d659467608a2be8c3045cb) | At most one claimed job per named queue in a batch; query and local-queue layers. | **Already supported.** Compared official `src/sql/getJobs.ts` with `crates/graphile-worker-queries/src/batch_get_jobs.rs`: lock candidate rows, then DISTINCT ON the queue (unnamed jobs retain their own ID), then update only retained claims. The limit is on candidates, so a smaller batch is intentional. `tests/upstream_sync.rs` exercises untouched siblings, priority, concurrent claimers, queue ownership and return. |
| [be38db6 / #623](https://github.com/graphile/worker/commit/be38db6319dc800f9f7a5b6be33dc9f499b2c586) | Avoid consuming existing task/queue identities; migrations and task registration. | **Already supported.** Official `sql/000020.sql` and `src/taskIdentifiers.ts` filter existing identities before INSERT and retain ON CONFLICT. Rust `m000021.sql`, retained by `m000022.sql`, and `task_identifiers.rs` do likewise; registration also deduplicates names. `tests/upstream_sync.rs` covers repeated add modes/registration; `tests/migrate/identity_upgrade.rs` covers both revision-20 histories, exhausted sequences, jobs/locks and original ledger rows. |
| [9fc851b / #627](https://github.com/graphile/worker/commit/9fc851b9f8087b309f5ff425b8d8f3e3497e3b45) | Reuse fetch SQL and per-worker helpers; query/context layers. | **Already supported / not applicable.** Rust `fetch_query_cache.rs` caches SQL text by Schema and fetch shape with a per-thread 128-entry bound; values and results stay uncached. Unit and integration tests exercise cache bounds, quoted schemas, flags, clocks and batch sizes. Compared upstream helpers/main/worker diffs: Rust shares owned context handles and static methods rather than rebuilding JavaScript closures. The Record-to-Map queue-name cache change has no corresponding Rust record cache. No new performance claim or benchmark is made. |
| [90ea860 / #557](https://github.com/graphile/worker/commit/90ea8609593345dcd782b1728dfea97fcae09a21) | Safe lifecycle event emission; lifecycle-hooks crate. | **Already supported.** `events/observer.rs` contains panics during handler construction and future polling and logs them while other observers continue. Its crate regression exercises both sites, another observer and repeated emission. Interceptors retain their documented control-flow role; Rust aborting panics cannot be caught. |
| [5a305b9 / #474](https://github.com/graphile/worker/commit/5a305b9424ddcee44ef38cc4aa9181bb756cc73b) | Optional batching/local queue and persistent-statement control; batchers, local queue, database wrapper. | **Already supported / intentional differences.** Existing completion/failure batchers and bulk insertion are retained. `SqlxDatabase::with_prepared_statements(false)` reaches direct executors and wrapper-created transactions; the driver-contract regression inspects server prepared statements. Raw SQLx executors retain their API. Retryable failure batching, typed hooks and explicit shutdown retain documented Rust contracts. Node's migration-19 compatibility marker must not replace Rust's applied migration 19. |
| [7de3c85 / #612](https://github.com/graphile/worker/commit/7de3c85e004c5c6aafc0908c56c4db139e013758) | Bigint failure IDs; failure/return queries. | **Already supported.** Official `src/sql/failJobs.ts` uses bigint[]; Rust `fail_job/batch.rs` binds I64Array and bigint[]. The queued/unqueued `batched_failure_accepts_job_ids_above_int32` regression checks IDs above i32::MAX, errors and queue unlocks. |
| [b94bcf8 / #544](https://github.com/graphile/worker/commit/b94bcf80ab496f63a0111107e7b8e052796b2f7f) | Cron retries, re-registration/backfill, cancellation and prepared-statement selection; cron runner/database. | **Already supported.** Compared official cron diff/current source with Rust runner/backfill and ADR 0001: indefinite 200ms-to-60s capped retries, healthy-tick reset, registration/backfill before resumed scheduling and interruptible waiting. The SQLx wrapper's persistent-statement option applies to cron queries too. `tests/cron/recovery.rs` uses a controlled clock and database faults. Current main also includes PR #537's UTC/local matching fix and four timezone regressions for live scheduling and backfill. |
| [bb03d1c / #543](https://github.com/graphile/worker/commit/bb03d1cd6bcabbab0e59c1db5c5c7651e46f2b5e) | Nested task identifiers in crontabs; parser/types. | **Already supported.** `crontab-parser/tests/crontab.rs::nested_identifiers_preserve_options_payload_and_following_entries` exercises options, JSON payload and subsequent entries. Strict whole-input rejection and location tests remain; compiled TaskHandler registration is the Rust API. |
| [#463](https://github.com/graphile/worker/pull/463) | Older queue cleanup correctness; WorkerUtils. | **Already supported.** Compared official `src/cleanup.ts` with Rust `graphile-worker-utils/src/types.rs`: NOT IN excludes null queue IDs and only unlocked queues are deleted. `tests/worker_utils_cleanup/job_queues.rs` covers jobs without queues and locked unused queues. |
| [0ec5c7e / #502](https://github.com/graphile/worker/commit/0ec5c7eb745635071ad8460466c5eb96d3fb2755) | Concurrent schema initialization; migration runner. | **Already supported.** Rust state/clash/runner paths retry recognized initialization clashes and retain other errors. `tests/migrate/install_schema.rs` exercises concurrent migrators and existing schemas; applied migration history is preserved. |
| [9dcec76 / #625](https://github.com/graphile/worker/commit/9dcec769c8e86ddfe8b05af4c5289e5a18382339) | PostgreSQL 14–18 CI matrix and Node versions; Test workflow. | **Already supported / not applicable.** This former delivery deferral is closed: PR #524 actually merged `.github/workflows/test.yml` with PostgreSQL [14,15,16,17,18], max-parallel=2 and fail-fast=false. The exact baseline Test run passed all five database jobs. Node 22/24/26 versions do not select compiled Rust runtimes. PostgreSQL 12 remains the documented minimum; the CI matrix does not prove a fresh full PostgreSQL 12 run. |
| [cea9d60 / #620](https://github.com/graphile/worker/commit/cea9d60e0341718b44b1851ff252b314527adbdf), [4207e7d / #569](https://github.com/graphile/worker/commit/4207e7d41b897fef09450dd35687ee73b5d76a1f), [91c950f / #558](https://github.com/graphile/worker/commit/91c950f7370aa2629107df1aa6a44b9f5985a1bd) | Attempts, array payload merging and queue cardinality documentation; Job types, scheduling and queue guides. | **Already supported.** Job/DbJob document the current consumed attempt; scheduling guide and latest SQL describe array concatenation and preserve_run_at's first-attempt condition; queue guide discourages random queue names. Existing upstream-sync and keyed-job regressions retain those conditions. |
| [f26dafe / #478](https://github.com/graphile/worker/commit/f26dafe622eac00c948218d06c203309bb4ee6de), [1a07517](https://github.com/graphile/worker/commit/1a07517fad8502773ddb0569ebdfd0c11599b8e7) | Restricted database roles and service-account guidance; operations docs. | **Already supported.** `docs/src/operations/migrations.md` distinguishes owner/migrator from restricted workers and explains grants/current-schema startup. No production privileges or data were touched. |

The September 18 downstream recovery and return fixes are also present in main:
`m000022.sql` retirement markers protect replaced/removed/permanently failed
jobs, lock-before-marker-read handles concurrent retirement/rescheduling, and
LocalQueue retains uncertain returns and their same-worker exclusion permits
across cancellation and exhausted retries. `tests/superseded_recovery.rs`,
`src/local_queue/return_tests.rs` and shutdown/TTL integration tests guard these
contracts. Migration SQL 1–22 is unchanged since PR #524. These are Rust
corrections, not upstream ports newly performed this week. Node v0.18.0 return
queries still do not consult Rust's retirement marker, as the compatibility
guide explicitly explains.

One documentation gap was clarified during this comparison. Official
`src/cron.ts::digestTimestamp` always uses UTC fields, regardless of `useNodeTime`.
Rust's current runner/schedule and backfill paths use UTC in the default mode,
but use local fields when `use_local_time(true)` is selected. PR #537 explicitly
preserves that local mode and fixes the default UTC mode. The cron, worker-option
and compatibility guides now describe this distinction rather than implying
all configurations share Node's UTC behavior. Existing `tests/cron/timezone.rs`
regressions exercise both modes, normal ticks, backfill, date boundaries and
recorded instants. No runtime code or public API is changed by this audit.

## Inapplicable changes, proposals and remaining limits

The historical ledger retains an individual commit link and explanation for
each JavaScript-only change. The applicable distinctions were rechecked:

- ESM/CommonJS exports, native TypeScript stripping, task-file extension order,
  cosmiconfig removal and graphile.config discovery operate on Node module/config
  loading. Rust uses compiled handlers and WorkerOptions; those loaders are absent.
- TypeScript LogLevel and import/type syntax changes use Node/TypeScript symbols;
  Rust keeps tracing levels and native types. Mixed ESLint/source changes were
  inspected beyond their titles: Error.cause metadata and the batcher
  throw-to-break adjustment add no retry contract to port to Rust Result paths.
- ECMAScript Symbol.asyncDispose does not implement async Rust Drop. Rust retains
  explicit awaited shutdown and its tested cleanup paths.
- Node executable-task stdout/stderr and AbortSignal/Promise helpers have no
  corresponding compiled-handler loader or JavaScript promise property. Rust
  cancellation uses shutdown futures and application state as documented.
- npm/Joi/webpack/yarn dependencies, Jest/Node OS matrices, website/funding copy,
  changesets and npm publishing affect tools not used by the Rust worker. Their
  individual historical entries remain inapplicable; dependency names alone
  are not evidence of a Rust security gap. Existing Rust CI and release tooling
  remain in place. No manual release or tag is part of this audit.

Open proposals were inspected separately and are **not merged behavior**:

| Proposal | Status and Rust comparison / next action |
| --- | --- |
| [#633](https://github.com/graphile/worker/pull/633) explicit migration entrypoints | Still open. Inspected its lib/runner/CLI patch: migration moves out of utility construction into explicit runtime entrypoints, with error cleanup. Rust WorkerUtils construction does not migrate, exposes `migrate`, and WorkerOptions initialization awaits migration before registration. Rust uses owned handles and Result propagation rather than JavaScript releaser arrays. Track for a future merged audit; no proposal is imported as a release requirement. |
| [#634](https://github.com/graphile/worker/pull/634) Joi dependency | Still open. The Node Joi dependency is absent from Rust worker crates. Recheck merge/source scope next audit. |
| [#637](https://github.com/graphile/worker/pull/637) task identifier cache recovery | Still open at head `3925e59a44d9f0d2d3fda2445053183d4c093ed3`. Compared its patch with official `src/taskIdentifiers.ts`, Rust `task_identifiers.rs` and `src/builder/init.rs`: Rust returns a fresh Result from registration, shares only a successful TaskDetails value and has no cached rejected promise or dynamic task-list cache completion to overwrite. No independent Rust defect was demonstrated; revisit if the proposal changes or merges. |

No applicable port is deferred in the reconstructed range. There remains a
**coverage limit**, rather than an implementation deferral: the absent September
5 report cannot establish an unknown pre-v0.16.6 gap list. Earlier behavior beyond
the explicit #463 cleanup revisit has not been fully certified. Discovery of a
specific older gap must extend the ledger; the inspected checkpoint does not
erase it. Full cross-language runtime, cross-OS, proxy and every mixed-version
deployment are not claimed. Recovering the older report or auditing the earlier
source range would extend that historical coverage. No new benchmark or
differential run is claimed.

## Validation and delivery

Local validation passed on Rust 1.99.0 with two build jobs and two test threads:

- `just lint`: formatting, all-target check and Clippy with warnings denied;
  repeated after the compatibility documentation update.
- `just test-docker`: **453 passed, zero failed, five ignored**, including
  documentation tests, on PostgreSQL 18.6. The disposable `postgres:latest`
  image resolved to
  `sha256:74935e72241653ca55e0414067e6d8763aceb8a810eb51b452253ec3dcfc4336`.
- All three Check-workflow OpenTelemetry feature checks passed.
- mdBook 0.4.52 build passed after the guide updates.
- Release-tooling Python regressions: 14 passed.

The initial full Docker attempt failed one batch-handler fixture during
migration with a pool-acquisition timeout. That attempt also contained a long
execution interruption and a corresponding PostgreSQL log timestamp gap; the
precise stall cause is not established. The unchanged original compiled test
passed a focused retry in 6.98 seconds, then the complete required Docker recipe
passed on a fresh disposable container. No runtime source, timeout, assertion
or test selection was changed to obtain the full successful run.

Exact-head delivery results are recorded in the durable run checkpoint and final
delivery report. No production queue or applied migration is changed. The
documentation PR has an empty body as required; evidence remains here.

Before this documentation update, exact baseline main CI was successful:
[Check](https://github.com/leo91000/graphile_worker_rs/actions/runs/37715163552),
[Test](https://github.com/leo91000/graphile_worker_rs/actions/runs/37715163530),
[Coverage](https://github.com/leo91000/graphile_worker_rs/actions/runs/37715163531).
Those historical runs are not a substitute for the report PR's exact-head checks
or post-merge main CI.
