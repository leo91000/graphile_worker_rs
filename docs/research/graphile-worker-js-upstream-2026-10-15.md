# Graphile Worker upstream audit — 2026-10-15

## Result and inspection checkpoint

There are no newly merged upstream changes and no missing applicable merged
port was identified in the retained audit range. Source comparison and
cross-driver validation exposed two downstream defects: `run_once()` returned
success after logging outcome-persistence errors, and tokio-postgres conversion
lost server diagnostic text. This audit fixes both and updates source evidence
and proposal tracking; it does not claim full runtime parity.

- Downstream baseline: `41d23d10ca2cfbd2a9ea968358011ad1ac86aa08`
  (`leo91000/graphile_worker_rs/main`).
- Live official upstream main and published stable `v0.18.0`:
  `4cda192c5df254392a1dff350e5d73f7d2c18a85`.
- Live maintenance branch `0.17.x` and peeled `v0.17.3`:
  `ca2cef1c8c6bf9d9c818d19f194f596971c212df`.
- Incremental inspected range:
  `4cda192c5df254392a1dff350e5d73f7d2c18a85..4cda192c5df254392a1dff350e5d73f7d2c18a85`
  (zero commits).
- Retained inspected range: peeled `v0.16.6`,
  `5650fbc4406fa3ce197b2ab582e08fd20974e50c` (exclusive), through the upstream
  checkpoint above (inclusive). A fresh first-parent enumeration found **105
  commits**, and a script verified that all 105 full commit URLs occur in the
  [September 17 per-commit ledger](graphile-worker-js-upstream-2026-09-17.md).
  Each merge entry represents its complete merge delta. That ledger, with the
  September 18, October 8 and present updates, remains the cumulative ledger;
  inspection coverage is separate from implementation coverage.

Fetched official branches and tags into a comparison checkout and inspected
CHANGELOG, release metadata, merged PRs, source and tests. The published stable
release remains v0.18.0, published September 8. **Merged but unreleased changes:
none** after that tag. Open PRs listed below are proposals, not released or
merged requirements. Release objects were not the only signal.

Read current AGENTS.md, architecture, all four ADRs, compatibility guidance,
prior audit reports, justfile and actual CI workflows in an isolated downstream
worktree. Durable state was absent, so the checkpoint was reconstructed from
committed reports and verified against fetched history. PRs
[#514](https://github.com/leo91000/graphile_worker_rs/pull/514),
[#524](https://github.com/leo91000/graphile_worker_rs/pull/524), and
[#547](https://github.com/leo91000/graphile_worker_rs/pull/547) landed in current
main. PR #547 has an actual APPROVED review at its final head and successful
checks. There is no open upstream-port PR to duplicate; dependency and release
PRs are outside this audit's delivery scope.

## Revisited behavior ledger

All 105 historical entries remain individually linked in the cumulative ledger.
The following source comparisons revisit the previously applicable ports,
deferred delivery and documented differences against this week's baseline.

| Upstream commit / PR | Behavior / Rust components | Current disposition and evidence |
| --- | --- | --- |
| [e9f3e3f / #622](https://github.com/graphile/worker/commit/e9f3e3fd6160c883b7d659467608a2be8c3045cb) | Named-queue serialization in batched claims; queries and LocalQueue. | **Already supported.** Compared official `src/sql/getJobs.ts` with Rust `batch_get_jobs.rs`: candidates are locked before DISTINCT ON queue identity, unnamed jobs retain their own ID, and only retained jobs consume attempts and ownership. Candidate limits may produce smaller batches. New Rust flag-routing predicates still precede the candidate limit. `tests/upstream_sync.rs` covers sibling state, priority, concurrent claimers and return. |
| [be38db6 / #623](https://github.com/graphile/worker/commit/be38db6319dc800f9f7a5b6be33dc9f499b2c586) | Preserve existing task/queue identities; migrations and registration. | **Already supported.** Upstream migration 20 and task registration filter existing identities before INSERT with ON CONFLICT. Rust migration 21, retained by 22, and `task_identifiers.rs` do the same, with deduplicated task names. `tests/upstream_sync.rs` and `tests/migrate/identity_upgrade.rs` cover repetitions, exhausted sequences and both revision-20 upgrade histories, retaining locks and ledger rows. |
| [9fc851b / #627](https://github.com/graphile/worker/commit/9fc851b9f8087b309f5ff425b8d8f3e3497e3b45) | Fetch SQL and helper allocation; query cache and context. | **Already supported / not applicable.** `fetch_query_cache.rs` keys text by Schema, complete flag-filter shape, explicit-clock mode and fetch shape, capped at 128 entries per thread. No values, connections or results are cached. Tests cover all eight filter shapes, clocks, batch modes and bounded tenant retention. Rust shares owned context handles rather than rebuilding JS closures; the JS Record-to-Map queue-name cache has no corresponding Rust record cache. No new speedup is claimed. |
| [90ea860 / #557](https://github.com/graphile/worker/commit/90ea8609593345dcd782b1728dfea97fcae09a21) | Safe event emission; lifecycle observers. | **Already supported.** `events/observer.rs` catches both handler-construction and future-polling panics while other observers continue. Existing crate regressions exercise both sites and repeated emission. Interceptors retain control flow; aborting panics remain outside containment. |
| [5a305b9 / #474](https://github.com/graphile/worker/commit/5a305b9424ddcee44ef38cc4aa9181bb756cc73b) | Batching, local queues and persistent statements; batchers/database. | **Already supported / intentional differences.** Existing batch/local-queue capabilities remain. SQLx's `with_prepared_statements(false)` reaches direct queries and wrapper-created transactions; driver-contract tests inspect server statements. Raw executors keep their own contract. Rust completion batches log persistence failure; failure batches fall back to per-job persistence. Neither is certified equivalent to Node retry/backpressure policy; see proposal #645 below. Rust typed hooks, explicit shutdown and applied migration 19 remain intentional. |
| [7de3c85 / #612](https://github.com/graphile/worker/commit/7de3c85e004c5c6aafc0908c56c4db139e013758) | Bigint failure IDs; failure queries. | **Already supported.** Rust `fail_job/batch.rs` binds I64Array and bigint[]. The upstream-sync regression exercises IDs above i32::MAX with and without named queues, checking errors and queue release. |
| [b94bcf8 / #544](https://github.com/graphile/worker/commit/b94bcf80ab496f63a0111107e7b8e052796b2f7f) | Cron retry, recovery/backfill, cancellation and statement selection. | **Already supported.** Rust CronRunner retries from 200ms to a 60s cap, re-registers/backfills after errors, resets after healthy ticks and races the entire runner against shutdown. ADR 0001 and deterministic `tests/cron/recovery.rs` document/test the contract. Statement opt-out also reaches cron queries. UTC/default and deliberate local-calendar behavior remain distinct, as documented last week and tested in `tests/cron/timezone.rs`. |
| [bb03d1c / #543](https://github.com/graphile/worker/commit/bb03d1cd6bcabbab0e59c1db5c5c7651e46f2b5e) | Nested cron task identifiers; parser. | **Already supported.** The crontab parser regression preserves options, JSON payload and following entries for nested identifiers. Whole-input rejection and location tests remain. Compiled TaskHandler registration is the intentional Rust API. |
| [#463](https://github.com/graphile/worker/pull/463) | Older queue cleanup correctness; WorkerUtils. | **Already supported, with subsequent Rust fixes.** Current `types.rs` uses a materialized set of non-null referenced queues and NOT EXISTS, retaining locked queues. It no longer uses the historical NOT IN implementation described last week. `maintenance.rs` serializes GC with inserts in a READ COMMITTED transaction and bounds lock acquisition. `tests/worker_utils_cleanup/` covers null references, locked queues, insert races, task refresh and reference semantics. |
| [0ec5c7e / #502](https://github.com/graphile/worker/commit/0ec5c7eb745635071ad8460466c5eb96d3fb2755) | Concurrent schema initialization; migrator. | **Already supported.** Compared upstream migration changes with Rust runner/state/clash logic: recognized initialization clashes are retried, unrelated errors propagate. `tests/migrate/install_schema.rs` covers concurrent migrators and existing schemas. Applied migration SQL has not changed since last week's baseline. |
| [9dcec76 / #625](https://github.com/graphile/worker/commit/9dcec769c8e86ddfe8b05af4c5289e5a18382339) | PostgreSQL and Node CI versions. | **Already supported / not applicable.** Test workflow still covers PostgreSQL [14,15,16,17,18], max-parallel=2 and fail-fast=false. The previous workflow authorization deferral was delivered in #524 and remains closed. Node version selection is inapplicable to compiled Rust. PostgreSQL 12 remains the documented minimum; no fresh full PG12 run is claimed. |
| [cea9d60 / #620](https://github.com/graphile/worker/commit/cea9d60e0341718b44b1851ff252b314527adbdf), [4207e7d / #569](https://github.com/graphile/worker/commit/4207e7d41b897fef09450dd35687ee73b5d76a1f), [91c950f / #558](https://github.com/graphile/worker/commit/91c950f7370aa2629107df1aa6a44b9f5985a1bd) | Attempt numbering, payload arrays, queue cardinality; job types and guides. | **Already supported.** Job/DbJob document consumed attempts; current SQL concatenates array payloads and preserves run_at only before an attempt; scheduling/queue guides describe these semantics and discourage random queue names. Existing keyed-job and upstream-sync tests remain. |
| [f26dafe / #478](https://github.com/graphile/worker/commit/f26dafe622eac00c948218d06c203309bb4ee6de), [1a07517](https://github.com/graphile/worker/commit/1a07517fad8502773ddb0569ebdfd0c11599b8e7) | Restricted roles and service accounts; migration operations guide. | **Already supported.** Operations docs distinguish migrator/owner privileges from restricted workers, grants and current-schema startup. No production privileges or queue data were touched. |

Rust migrations 1–22 are unchanged relative to last week's baseline. Revision 22
still protects replaced/removed/permanently failed jobs from automatic attempt
restoration, locks before reading retirement markers and clears markers on
explicit rescheduling. Rust LocalQueue retains uncertain returns and same-worker
claim exclusion through cancellation/retries. Existing retirement, upgrade,
shutdown and LocalQueue fault tests remain; these downstream capabilities must
not be overwritten by Node SQL. Node v0.18.0 return queries do not consult Rust's
retirement marker. Mixed-version runtime parity is not claimed.

The baseline also includes accepted routing and optional allowlists (ADRs
0003/0004), GC concurrency fixes, heartbeat retries, slot draining and direct
completion error propagation. Source was inspected so historical evidence is not
mistaken for current implementation. These are landed downstream changes, not
new upstream ports in this audit.

## Downstream outcome-error correction

Comparing the direct release paths for open upstream proposal #643 revealed
that Rust's `run_once()` discarded their ReleaseJobError after logging it.
Continuous `Worker::run()` already propagated it. The dismissed review on
downstream [PR #557](https://github.com/leo91000/graphile_worker_rs/pull/557)
also identified this gap; inspecting current source confirmed
that it remained unfixed. A dismissed review is not evidence that a finding is
invalid.

`run_once()` now retains the first release error, continues draining its
concurrent stream, and returns that error with the original job ID and database
source chain. Successfully persisted task failures still follow their existing
retry behavior and do not become worker errors. Named-queue follow-up claims
retain their existing behavior. No public signature or migration changes.

Four integration regressions inject SQLSTATE 40001 while persisting completion
or failure, each with and without a named queue. A nontransactional sequence
observes the rejected statement while a healthy sibling waits on an explicit
gate. The checks require sibling completion before return, preserved SQLSTATE
and job identity, no false outcome hooks, and intact failed job/queue locks.
They use isolated UUID databases with panic-safe fixture cleanup. Validation
results, including the pre-fix reproduction, are recorded below.

Cross-driver validation exposed a second downstream defect: the tokio-postgres
adapter copied the outer driver's generic "db error" display instead of the
server message. All four new regressions caught that loss even though the
SQLSTATE and run-once error propagated. Conversion now retains the outer context
and server diagnostic together, preserving the SQLSTATE. The shared driver
contract also injects a server error through direct execution and transaction
fetching and requires both its message and code. No assertion was weakened to
accommodate a driver difference.

The comparison also found stale option documentation claiming that retryable
failures bypass batching. Current `release/failure.rs` batches ordinary failures
regardless of remaining attempts and bypasses batching for replacement payloads.
The builder and configuration docs now describe that existing behavior and its
backoff timestamp at persistence; runtime batching behavior is preserved.

## Delivery feedback and precise internal errors

The initial head `184fe9c7c22ca352f4644d3847dc9241fa9fe641` received an actual
APPROVED review with no actionable inline findings. Its patch coverage check
identified one missed line: the closing branch at `run_once.rs:59`. The pinned
Codecov line report confirmed that this was the impossible non-release-error
branch: `run_and_release_job` never fetches jobs, but declared ProcessJobError,
whose other variant represents fetch failures.

The private execution helper now returns ReleaseJobError, matching the errors
it can actually produce. Callers retain their existing ProcessJobError
conversions, including fetch errors in the paths that do fetch. Run-once keeps
the same public error chain and logging without an unreachable variant guard.
Coverage thresholds, exclusions and assertions are unchanged. Regression
fixtures and the shared contract helper also gained Rust documentation in
response to the review's docstring warning. The suggestion to populate the PR
description is inapplicable because the owner explicitly requires an empty
body; the detailed evidence stays in this report. Approval and checks on that
initial head do not establish approval or validation of the updated head.

The next review at `6f0148340d4cb41a64b8c462cbad4fbbc2e2fa94` was COMMENTED,
not approved. Its outside-diff finding correctly identified the failure batcher's
closed-channel fallback: `fail_job_direct` reduced a query error to `false`,
`FailureBatcher::fail` returned unit, and the release caller reported success.
The direct helper and closed-channel caller now preserve the query Result,
and release maps it to the original job's ReleaseJobError. Asynchronous batch
flushing still collects per-job success booleans and emits hooks only for
persisted failures; its existing policy is preserved.

A new regression deterministically closes the failure channel while its handler
is gated, then injects SQLSTATE 40001 on the failure update. It covers both
continuous `run()` and `run_once()`, each with and without a named queue,
requiring the original diagnostic/job ID, no false failure hook, and intact
job/queue ownership. Before this correction, it failed because the worker
returned Ok after the database rejected persistence. The prior head's local
required recipe had completed with 499 passed, zero failed and six ignored;
this additional correction passed fresh `just lint` and `just test-docker`
(500 passed, zero failed, six ignored), all five batcher tests under async-std,
and the 19-test tokio-postgres run-once suite. Fresh-head CI and actual review
remain delivery gates; the final checkpoint/report records their outcomes.
The previous docstring warning passed at 90.91%; the PR-body suggestion remains
inapplicable under the owner's explicit empty-body instruction.

## Open proposals, inapplicable changes and unresolved coverage

Each proposal patch and its tests were read. All were **open**, not merged, at
inspection. Recheck status and changed source on the next audit.

| Proposal and inspected head | Rust comparison / disposition |
| --- | --- |
| [#633](https://github.com/graphile/worker/pull/633), `13679fe51b7751597db8dc1ffebaf8a69f58e2cb` | **Track.** Explicit migration entrypoints and cleanup on initialization errors: WorkerUtils construction already does not migrate, exposes `migrate`, and WorkerOptions initialization awaits migration before registration. Rust owned handles/Result propagation replace JS releaser arrays. No newly demonstrated defect. |
| [#634](https://github.com/graphile/worker/pull/634), `29b73dcc5dc817399f7dcb5c564f701639a3228d` | **Not applicable / track merge.** Patch changes only Node's Joi dependency graph in yarn.lock; worker Rust crates do not use Joi. |
| [#637](https://github.com/graphile/worker/pull/637), `3925e59a44d9f0d2d3fda2445053183d4c093ed3` | **Track.** Rejected-promise cache and stale task-list completion: `get_tasks_details` returns a fresh Result, and init shares only success. Current cleanup refresh compares the snapshot under the write lock, retries changed snapshots up to three times, then returns a conflict without overwriting concurrent registration. No cached rejected promise. |
| [#640](https://github.com/graphile/worker/pull/640), `74ccbfd8c8780e5ca9b81ed3ee5924f8c87e0375` | **Track.** Alternate cache-rejection fix; same fresh-Result comparison as #637. No JS promise cache exists to invalidate. |
| [#642](https://github.com/graphile/worker/pull/642), `6c6b57897d303db3713e9b9bf3c21d4e3666f725` | **Not applicable.** Only upstream sponsor copy changes; this does not establish Rust sponsorship or change worker behavior. |
| [#643](https://github.com/graphile/worker/pull/643), `11201b5e52a64f533b62a99c1d01bfdab8e608cc` | **Track; independent Rust correction implemented.** Direct completion/failure paths await ReleaseJobError; continuous run and completion fallback propagate it after downstream #557, but baseline `run_once()` discarded it. This audit closes that gap with four fault-injection regressions. The existing batcher shutdown regression checks fallback errors. Asynchronous batch failures still log rather than propagate to the worker; the open proposal is not treated as merged behavior. |
| [#644](https://github.com/graphile/worker/pull/644), `7db0b5ff0cd36d2283b65e0eaee20bbad695a9be` | **Track.** Serialize keyed scheduling through ordered transaction advisory locks. Rust migration 19 introduced sorted advisory key locks, retained through revision 22; the bulk locked-key concurrency regression checks both return values and one surviving key. Rust also retains its GWNOR missing-result guard. Upstream proposal uses a different advisory hash/key namespace, so this is not a claim that mixed Node/Rust producers share the same lock. No new migration is imported. |
| [#645](https://github.com/graphile/worker/pull/645), `0a77c0704669a0a06adda3c5ff504c8507889e58` | **Track, behavior differs.** Live upstream `main.ts` exits the batch retry loop on first error and does not clear the resolved backpressure deferred. The proposal adds retries for selected SQLSTATEs and resets backpressure. Rust uses bounded channels instead of that JS deferred/refcount protocol, logs failed completion batches, and falls back to single-job failure persistence. It has no matching break/deferred bug, but also has no equivalent configured batch retry/backoff policy. Do not label that retry capability already supported. Re-evaluate the final merged policy and add driver-aware fault regressions when porting it. No proposal is treated as merged upstream behavior. |

The individual JavaScript-only dispositions in the September 17 ledger remain:
ESM/CommonJS exports, TS stripping/extension order and config discovery operate on
module loaders absent from compiled Rust; TypeScript log/type/import symbols do
not replace Rust tracing/types; Symbol.asyncDispose does not provide async Rust
Drop (explicit awaited shutdown remains). Executable-task stdout/stderr and JS
Promise/AbortSignal helpers have no compiled-handler loader counterpart. npm,
Joi, webpack, yarn, Jest/Node OS matrices, funding/website copy, changesets and
npm publishing apply to different tools. Mixed source diffs, including the
ESLint batch-loop change, do not establish a retry capability merely from a
feature name. PostgreSQL behavior and operational documentation were considered
separately above. No manual release or tag is authorized or performed.

**No applicable merged-upstream port remains deferred in the reconstructed
range.** The missing September 5 report is still a coverage limit: the file is
absent and a fetched downstream history search for its path found no version to
recover. Unknown pre-v0.16.6 gaps beyond the explicitly revisited #463 cleanup
cannot be certified. Recovering that report or auditing that earlier source
range extends coverage; retaining the current inspected SHA must not erase this
limit. Cross-language runtime, every proxy/OS, and all mixed-version deployment
combinations remain uncertified. No new differential or benchmark result is
claimed for this audit.

## Validation and delivery

The baseline's exact-main Check, Test and Coverage workflows succeeded:
[Check](https://github.com/leo91000/graphile_worker_rs/actions/runs/38586304375),
[Test](https://github.com/leo91000/graphile_worker_rs/actions/runs/38586304449),
[Coverage](https://github.com/leo91000/graphile_worker_rs/actions/runs/38586304178).
These runs are evidence for the baseline, not a substitute for the report PR's
checks. The current CI includes all five PostgreSQL versions, TLS 1.2/1.3 jobs,
four driver/TLS coverage variants, three OpenTelemetry feature checks, Python
release-tool regressions and mdBook. Existing release automation remains intact.

Local validation uses Rust 1.99.0 and disposable PostgreSQL 18.6 fixtures:

- The four new regressions failed against the exact baseline run-once runtime
  blob `775ceb5bcb51249e561f49de010abf9b36437cd0`, each because it returned Ok.
- After the run-once correction, all 19 run-once tests passed with SQLx.
- The tokio-postgres run initially passed 15 tests and failed four specifically
  on lost server diagnostic text. After fixing conversion, all 19 passed; its
  five shared driver-contract tests also passed, including the new direct and
  transactional server-error assertions.
- All four fault regressions passed with SQLx's async-std feature configuration.
- `just lint` passed again after the adapter correction, including Clippy with
  warnings denied. All three OpenTelemetry feature checks passed on that source.
- The complete `just test-docker` recipe passed **499 tests, zero failures, six
  ignored**, including doc tests. The delivery rerun after the adapter update also passed;
  exact completed delivery results are retained in the durable checkpoint and
  final report. No failing assertion, timeout, or test selection was relaxed.
- mdBook 0.4.52 built the guides; 14 Python release-tooling regressions and all
  six TLS dependency-isolation checks passed.

The initial test-fixture build needed a missing Debug derive; that was corrected
before the four baseline behavioral failures were captured. PostgreSQL's pinned
image digest is
`sha256:74935e72241653ca55e0414067e6d8763aceb8a810eb51b452253ec3dcfc4336`.
No new differential or benchmark run is claimed.

A four-thread delivery attempt failed the existing
`slot_failure::edge_cases::heartbeat_continues_while_slots_drain_after_a_failure`
check: the sweep reported the draining worker as stale. That fixture uses a
20ms heartbeat, 200ms stale threshold and real-time waits. The unchanged compiled
test passed a focused retry in 2.78s. Inspection confirmed that recovery tasks
still stop after slot/batcher draining; those runtime paths and the fixture are
unchanged by this audit. The precise stall cause was not established. Delivery
returned to the original two-thread limit and two full required recipe runs
passed on fresh databases; neither the liveness assertion nor its timing is relaxed.

Builds and full delivery runs use two jobs/test threads; standalone tokio-postgres
tests use one. Exact-head delivery
results are also retained in the durable checkpoint/final report. The correction retains
public signatures, existing assertions and applied migrations. No production
data changes are part of this audit; the PR body is empty and detailed evidence
stays here.
