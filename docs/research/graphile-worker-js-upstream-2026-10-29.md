# Graphile Worker upstream audit — 2026-10-29

## Result and inspection checkpoint

No new merged upstream implementation or missing applicable Rust port was found
in this week's inspected range. This is a documentation-only audit, not a claim
of full cross-runtime parity. The previous braces development-dependency
deferral is closed because the advisory was withdrawn, not because a patched
package was installed.

| Reference | Verified SHA / range |
| --- | --- |
| Downstream `leo91000/graphile_worker_rs/main` baseline | `1c56b1ac09b5f1c3e584b52944223fe877ccb9de` |
| Previous and current upstream inspection checkpoint | `7a2405bdd3b6322fb6b0d48dc0543bcc89166930` |
| Incremental range | `7a2405bdd3b6322fb6b0d48dc0543bcc89166930..7a2405bdd3b6322fb6b0d48dc0543bcc89166930`: zero commits |
| Published stable v0.18.0, peeled tag | `4cda192c5df254392a1dff350e5d73f7d2c18a85`, published 2026-09-08 |
| Merged but unreleased | `4cda192c5df254392a1dff350e5d73f7d2c18a85..7a2405bdd3b6322fb6b0d48dc0543bcc89166930`: one first-parent merge, two reachable commits |
| Maintenance `0.17.x` / peeled v0.17.3 | `ca2cef1c8c6bf9d9c818d19f194f596971c212df`; no commits outside main |
| Reconstructed cumulative ledger | v0.16.0 (`188c330e76e036bf2671e772c9b86b93beb5b31b`, exclusive) through current upstream main: 147 first-parent deltas |

Fetched official live Git refs/tags, branches, release objects and notes,
CHANGELOG, merged/open/closed PR metadata, complete proposal patches and current
source/tests. The upstream clone is comparison-only; no upstream write was made.
No merge has occurred since the previous checkpoint. Release objects alone
were not used as the inventory. The stable-to-main complete diff remains only
24 changed lines in yarn.lock, with no src/, SQL, tests or documentation delta.

The node had no local state directory. Reconstructed the checkpoint from
committed reports, fetched full history, checked ancestry and enumerated every
first-parent delta. All 147 full SHAs have individual ledger entries: 41 in the
[October 22 broadened v0.16.0..v0.16.6 ledger](graphile-worker-js-upstream-2026-10-22.md#broadened-published-stable-ledger-v0160v0166),
105 in the [September 17 v0.16.6..v0.18.0 ledger](graphile-worker-js-upstream-2026-09-17.md),
and one unreleased delta below. Those individual entries remain part of this
cumulative ledger; the current source comparisons below update their behavioral
dispositions. Counting covered SHAs verifies inventory completeness within this
range, not implementation parity by itself.

The September 5 report is absent from main and a full fetched history search
for its path. The last audit already broadened to v0.16.0 to address that missing
checkpoint. No history divergence or uncovered commit was found within the
reconstructed range. Pre-v0.16.0 behavior remains outside certified coverage;
the inspected SHA must not erase that limit. Recovering earlier evidence or
auditing the earlier source range is the condition for extending coverage.

Read AGENTS.md, architecture, ADRs 0001–0005, compatibility and operational
guidance, justfile, toolchain pin and actual workflows. Work uses an isolated
worktree. No competing upstream-sync PR was open; unrelated dependency and
release PRs were left untouched. No manual tag, release or queue operation was
performed.

## Merged but unreleased ledger

| Upstream change | Behavior / affected component | Current disposition and evidence |
| --- | --- | --- |
| [7a2405b / #634](https://github.com/graphile/worker/commit/7a2405bdd3b6322fb6b0d48dc0543bcc89166930), branch [29b73dc](https://github.com/graphile/worker/commit/29b73dcc5dc817399f7dcb5c564f701639a3228d) | Joi 17.11.0 → 17.13.7, address 4.1.4 → 4.1.5, dependency ranges/checksums. Node validation/development dependency security fixes. | **Inapplicable to Rust.** Complete merge diff changes only yarn.lock. Cargo and the independent admin UI manifests/lockfile do not use Joi, its address validator or the upstream website dependency graph. Rust typed configuration does not execute these packages. Meaningful dependency security change for Node, not a newly published stable queue change. |

## Published-stable behavior rechecked on current main

The earlier ports actually landed: [#524](https://github.com/leo91000/graphile_worker_rs/pull/524)
at `4d2e979`, [#547](https://github.com/leo91000/graphile_worker_rs/pull/547)
at `fd8411c`, [#568](https://github.com/leo91000/graphile_worker_rs/pull/568)
at `607b218`, and [#576](https://github.com/leo91000/graphile_worker_rs/pull/576)
at `f22bfd931b552e7805844c055736f9411a4b5e6c` are ancestors of the baseline.
The latter contains the October 22 report, SQL retries, queue-name helpers,
cleanup keep list, run-once fetch-stop/claim-return fixes and source-map fix.
The subsequent diff to current main contains only Cargo/example/documentation
changes; these port implementations and tests remain present. Source was read
directly, including the final follow-up code, rather than trusting PR titles.

| Upstream commit / PR | Behavior and affected components | Disposition / current Rust evidence |
| --- | --- | --- |
| [4e44699 / #430](https://github.com/graphile/worker/commit/4e44699d8f72113f37ba8089776a859b57d76d2b), [86bfcc8 / #439](https://github.com/graphile/worker/commit/86bfcc841a06e052b48d1c2464d70080e4b0e720), [93e6e60 / #445](https://github.com/graphile/worker/commit/93e6e607f8bb37bdabc4faa44c62001b062480c4), [3fa9f06 / #451](https://github.com/graphile/worker/commit/3fa9f068069d131cc42f82427177c0b7155af668), [5a305b9 / #474](https://github.com/graphile/worker/commit/5a305b9424ddcee44ef38cc4aa9181bb756cc73b) | Serialization/deadlock/connection retries and exponential jitter; database and driver execution. | **Already supported with documented Rust policy.** `database/src/retry.rs`, `database.rs`, SQLx and tokio-postgres executors match the current upstream codes, 50ms/3s initial delays, 5s/120s caps, multiplier 1.5 and 0.5–1.5 jitter. The default 100 total attempts is opt-in. No final useless sleep; no transaction/commit/LISTEN replay, and ambiguous execution I/O is not retried. Deterministic policy tests and `tests/sql_retry.rs` cover bindings, cancellation, exhaustion, transaction ownership and actual rollback/persistence. ADR 0005 retains these deliberate differences from automatic Node retries. |
| [e855a5a / #428](https://github.com/graphile/worker/commit/e855a5a5d519927054b108e089ed53112c15c281) | Queue-name lookup; query and context APIs. | **Already supported / Rust adaptation.** `get_queue_names.rs` maps bound int IDs in input order with duplicates/missing entries. `WorkerContextQueueExt` resolves the running job's queue. `tests/queue_names.rs` verifies fresh results, errors, quoting and missing IDs; retry integration handlers use the context helper. Immediate Result/Option replaces Node's timer/cache and missing-cache-entry rejection; no cache-performance equivalence is claimed. |
| [6ef086c / #465](https://github.com/graphile/worker/commit/6ef086c15bf17d363ebffd10d821dbbab2362adc) | Additional cleanup keep identifiers; utils and CLI. | **Already supported.** `cleanup_with_keep` merges explicit/local names, sorts/deduplicates, does not register extras or mutate the task cache; CLI `--keep-task` replaces JS task-directory discovery. `explicit_keep_preserves_external_identity_without_changing_local_cache` checks identity/cache preservation. |
| [0d71581 / #415](https://github.com/graphile/worker/commit/0d7158195bcf136154c94fab1eaac28d59b0c933), [c0753d4 / #463](https://github.com/graphile/worker/commit/c0753d457f3473c5b3caa01ade2c8efff7d20457), [e4e4ddc / #447](https://github.com/graphile/worker/commit/e4e4ddc6696dc41ae5b7b6489a8b9f1082735ce7) | Null/lock-safe cleanup and safety documentation; maintenance SQL and task cache. | **Already supported with stronger Rust safeguards.** `utils/src/types.rs` retains referenced/locked queues; `maintenance.rs` serializes GC with inserts under READ COMMITTED, bounds lock waits and refreshes a matching cache snapshot only. Cleanup integration tests cover null references, live locks, insertion races and concurrent registration. Guides retain cross-worker task-cache warnings. |
| [e9f3e3f / #622](https://github.com/graphile/worker/commit/e9f3e3fd6160c883b7d659467608a2be8c3045cb) | One job per named queue in a batch; locking/concurrency. | **Already supported.** `batch_get_jobs.rs` locks candidates before DISTINCT ON queue identity, preserves separate unnamed jobs and filters before limiting/claiming. `tests/upstream_sync.rs` verifies priority, untouched siblings, attempts and concurrent claimers. LocalQueue keeps these rules. |
| [be38db6 / #623](https://github.com/graphile/worker/commit/be38db6319dc800f9f7a5b6be33dc9f499b2c586) | Avoid consuming task/queue identities for known names; migration and registration. | **Already supported.** Migration 21 and current revision 22 filter missing names before INSERT; `task_identifiers.rs` does likewise. Upstream-sync identity/exhaustion tests and both revision-20 upgrade fixtures remain. Applied migrations are not rewritten. |
| [9fc851b / #627](https://github.com/graphile/worker/commit/9fc851b9f8087b309f5ff425b8d8f3e3497e3b45) | SQL/helper allocation optimization. | **Already supported / Rust design.** `fetch_query_cache.rs` caches SQL text only, bounded to 128 entries per thread and separated by schema, filter shapes, time and fetch mode. Values remain bindings; contexts own Rust handles. Upstream-sync tests vary schema/flags/time/batch size. No new throughput equivalence is claimed. |
| [90ea860 / #557](https://github.com/graphile/worker/commit/90ea8609593345dcd782b1728dfea97fcae09a21) | Safe observer emission; hooks and lifecycle. | **Already supported.** Lifecycle observer wrapper catches construction and polling panics, logs them, and preserves other observers/cleanup; tests exercise both sites and repeated emissions. Interceptors retain their distinct flow-control contract; aborting panics and application side effects are not rolled back. |
| [5a305b9 / #474](https://github.com/graphile/worker/commit/5a305b9424ddcee44ef38cc4aa9181bb756cc73b) | Batching and prepared-statement opt-out. | **Supported with documented policy differences.** Completion/failure batchers use bounded channels. Direct fallback preserves persistence errors; asynchronous failures retain logging/per-job fallback rather than Node's fatal-pool policy. `SqlxDatabase::with_prepared_statements(false)` reaches direct and wrapped transaction queries. Optional SQL retries apply to pooled batch statements; that is distinct from the open batch-loop proposal. Existing batcher, driver-contract and statement tests remain. |
| [7de3c85 / #612](https://github.com/graphile/worker/commit/7de3c85e004c5c6aafc0908c56c4db139e013758), [58d1ed2 / #613](https://github.com/graphile/worker/commit/58d1ed2d16a30582244d2234d77de208e587bfdd) | Failure job-ID casts. | **Already supported.** Failure bindings use I64Array/bigint[]; `batched_failure_accepts_job_ids_above_int32` checks persisted failure/attempts and queue release above i32::MAX. The branch fix is included in main's merge delta. |
| [b94bcf8 / #544](https://github.com/graphile/worker/commit/b94bcf80ab496f63a0111107e7b8e052796b2f7f), [bb03d1c / #543](https://github.com/graphile/worker/commit/bb03d1cd6bcabbab0e59c1db5c5c7651e46f2b5e) | Cron recovery/backfill and nested identifiers. | **Already supported.** Crontab runner retries registration/backfill with 200ms→60s capped backoff, resets after healthy ticks and races database/backoff against shutdown. Deterministic `tests/cron/recovery.rs`, timezone/backfill/boundary and parser tests remain. ADR 0001 and compatibility docs retain Rust's intentional local-calendar mode; the default uses UTC like Node. |
| [0ec5c7e / #502](https://github.com/graphile/worker/commit/0ec5c7eb745635071ad8460466c5eb96d3fb2755) | Concurrent schema initialization. | **Already supported.** Migrator clash classification retries recognized concurrent-init conflicts and propagates other errors. `tests/migrate/install_schema.rs` covers concurrency and existing schemas; fresh and Node/Rust revision-20 upgrades retain assertions. |
| [9dcec76 / #625](https://github.com/graphile/worker/commit/9dcec769c8e86ddfe8b05af4c5289e5a18382339) | PostgreSQL/Node CI versions. | **Already supported / runtime-specific.** Current Test workflow runs PostgreSQL 14–18, max-parallel=2, fail-fast=false. Rust keeps the documented PostgreSQL 12 minimum. Node versions/Jest module-loader OS tests do not apply to compiled Rust; no fresh full PG12 certification is claimed. |
| [cea9d60 / #620](https://github.com/graphile/worker/commit/cea9d60e0341718b44b1851ff252b314527adbdf), [4207e7d / #569](https://github.com/graphile/worker/commit/4207e7d41b897fef09450dd35687ee73b5d76a1f), [91c950f / #558](https://github.com/graphile/worker/commit/91c950f7370aa2629107df1aa6a44b9f5985a1bd) | Attempt numbering, array merging and queue-cardinality documentation. | **Already supported.** Job types document consumed attempts; keyed SQL concatenates unlocked array payloads and preserves run_at only before an attempt. Keyed/batch tests and scheduling/queue guides retain these rules and discourage random queue names. Locked replacement retirement remains a separate documented rule. |
| [f26dafe / #478](https://github.com/graphile/worker/commit/f26dafe622eac00c948218d06c203309bb4ee6de), [1a07517](https://github.com/graphile/worker/commit/1a07517fad8502773ddb0569ebdfd0c11599b8e7) | Restricted roles/service accounts. | **Already supported.** Operations migration guide separates schema owner/migrator privileges from restricted worker roles and current-schema startup. No production grants or data were touched. |

The individual remaining packaging/runtime/website rows in the cumulative
ledgers retain their concrete dispositions: compiled TaskHandler registration
does not load ESM/CommonJS/TypeScript files or child-process tasks; Rust builders
do not discover cosmiconfig/graphile.config; tracing, typed hooks and explicit
awaited shutdown replace Node log/type/EventEmitter/async-disposal contracts.
Upstream npm/yarn/Jest/publishing/site/funding changes operate on tools and copy
absent from the Rust worker. Mixed SQL/documentation changes were considered
separately above. Similar names are not evidence of equivalent behavior.

## Open and closed proposals: tracking only

Every listed patch, including tests where present, was read at the live head.
None is merged behavior. #646 changed since last week; #649 is newly tracked.

| Proposal / inspected head | Behavior, affected components and current disposition |
| --- | --- |
| [#633](https://github.com/graphile/worker/pull/633), `13679fe51b7751597db8dc1ffebaf8a69f58e2cb` | **Open; track.** Explicit migration entrypoints/error cleanup. WorkerUtils construction is migration-free, migrate is explicit, WorkerOptions init migrates before registration. Owned Rust handles/Results replace JS releaser arrays. No newly demonstrated missing behavior. |
| [#637](https://github.com/graphile/worker/pull/637), `3925e59a44d9f0d2d3fda2445053183d4c093ed3` | **Open; track.** Rejected/stale task-lookup cache. Rust fetches fresh Results; init shares successful details only. Cleanup refresh compares snapshots under its write lock and returns a bounded conflict rather than overwriting concurrent registrations. Existing cache/race tests remain. |
| [#640](https://github.com/graphile/worker/pull/640), `74ccbfd8c8780e5ca9b81ed3ee5924f8c87e0375` | **Closed unmerged.** Alternate rejected-cache proposal. Same Rust comparison as #637; closure is not a merged implementation. |
| [#642](https://github.com/graphile/worker/pull/642), `6c6b57897d303db3713e9b9bf3c21d4e3666f725` | **Open; inapplicable.** Upstream sponsor list only; does not establish downstream sponsorship or queue behavior. |
| [#643](https://github.com/graphile/worker/pull/643), `11201b5e52a64f533b62a99c1d01bfdab8e608cc` | **Open; track.** Unbatched outcome-error reporting. Rust awaits direct completion/failure, preserves job ID/SQLSTATE/diagnostic and drains active jobs before returning run-once errors. Closed-channel fallback errors propagate. Existing persistence fault regressions remain. Asynchronous batch-error policy still differs. |
| [#644](https://github.com/graphile/worker/pull/644), `7db0b5ff0cd36d2283b65e0eaee20bbad695a9be` | **Open; track.** Keyed scheduling serialization. Rust revision 22 retains sorted advisory locks from revision 19, locks existing rows and keeps GWNOR for missing results; keyed contention regression checks returned jobs and one surviving key. The advisory namespaces differ; mixed Node/Rust producer coordination is not certified. |
| [#645](https://github.com/graphile/worker/pull/645), `0a77c0704669a0a06adda3c5ff504c8507889e58` | **Open; track with differences.** Node batch-loop retries and backpressure-deferred reset. Rust bounded channels have no matching deferred/refcount protocol. Optional pooled SQL retries provide a separate policy, while async completion logging/failure per-job fallback remains distinct from fatal-pool shutdown. Reassess final policy/fault cases if merged. |
| [#646](https://github.com/graphile/worker/pull/646), `45b79c00f812298d6bd838d17563d6c7f44aac3d` | **Open; changed head, track with compatibility decision.** Keyed insert/claim race, array retry exemption, in-place updates of unlocked exhausted keyed jobs, deletion of superseded scalar jobs in force-unlock/stale-reset/return paths and corresponding docs/tests. Rust's ordered key and row locks already address its keyed race. Revision-22 retirement markers deliberately retain retired rows, release ownership without restoring attempts and allow explicit rescheduling. Like stable upstream, its add_jobs retires unavailable keyed rows, including exhausted unlocked rows, rather than adopting this proposal's locked-only retirement condition. Markers distinguish ordinary final attempts from retirement rather than inferring retirement from key/payload/error fields. The proposal's changed retirement/deletion rules are absent from stable/main and are not imported into applied SQL. Re-evaluate final merged SQL, partial array failures, both recovery paths, upgrade history and cross-runtime interaction when it lands (ADRs 0002/0005). |
| [#647](https://github.com/graphile/worker/pull/647), `1bf3da11c43711975f9cfd4f75a53f2b43df0c32` | **Open; dependency-only tracking.** follow-redirects, handlebars, http-cache-semantics, Joi, shell-quote and undici changes do not occur in the Rust worker or independent UI build graph. The overlapping source-map-js fix already landed at 1.2.2 in #576; its oversized-offset/valid-map regression is in Check CI and passed locally. This proposal still is not a merged queue port. |
| [#648](https://github.com/graphile/worker/pull/648), `91ef5723e730b48de6b563b04cd2d1800e6c64b2` | **Open; track; independent Rust correction already landed.** Runner/CLI error propagation. Continuous Rust run preserves the original source error through draining/deregistration. #576 retains run-once fetch errors, stops stream and named continuations, and awaits/returns committed unstarted claims. `tests/run_once/fetch_errors.rs` includes initial/stream/follow-up failures, both in-flight directions and rejected-return SQLSTATE injection. Original error and claim ownership remain observable. |
| [#649](https://github.com/graphile/worker/pull/649), `9b3b5968d77d715cea2b4d2d7a6237d2f6fcdf5d` | **Open; newly tracked documentation.** Dedicated local-queue options, tuning, tradeoffs and events page. Rust already has `guides/local-queue.md`, worker options/queue guides and typed hooks. Compared with actual `local_queue/config` and `refetch_delay.rs`: Rust enables it through an Option, uses size 100 per queue, adds queue_count, and jitters refetch duration by 0.5–1.0 rather than Node's 0.5–1.5. Rust's documented return/recovery rules differ from Node's four-hour/Pro copy. Keep Rust-specific docs; do not copy incompatible defaults or claim the proposal is merged. |

## Deferred items and compatibility limits

1. **Braces deferral closed after recheck.**
   [GHSA-vfj7-8cjw-p6xm](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm)
   was withdrawn on 2026-10-23 at 12:53:02Z because it was assessed as not
   practically exploitable. The live npm registry still reports 3.0.3; the
   existing lockfile still uses it through micromatch/@parcel/watcher/Tailwind.
   Fresh locked installation plus `npm audit --json` reports zero vulnerabilities.
   No patched-version claim, downgrade, override or pin change is made. The
   source-map-js 1.2.2 fix and bounded parser regression remain independently
   valid. The previous four propagated alerts are not silently carried forward
   as active findings or mistaken for a code fix.
2. **Pre-v0.16.0 coverage remains uncertified.** Attempted recovery of the
   September 5 report found no file/history. The reconstructed range is complete
   from v0.16.0 exclusive, not from the first upstream release. Extending it
   requires source comparison of the older range or recovered verifiable
   evidence. No identified applicable merged port within the 147-delta range
   remains deferred.
3. **Open proposals retain their individual conditions above.** In particular,
   batch fatal-error/retry policy and locked-array retirement are different,
   tracked contracts. Their final merge is the trigger for a fresh comparison;
   their current size is not a deferral reason.
4. **Existing runtime/deployment limits remain.** Rust SQL retries are opt-in;
   transaction retries belong to callers; missing queue names use Option;
   observer isolation does not cover aborting panics; local-calendar cron is
   intentional; Node return queries do not consult Rust retirement markers.
   Revisions 1–22 remain byte-for-byte unchanged. Equal Node/Rust migration
   numbers, different advisory-lock namespaces, every proxy/OS/PG12 combination
   and mixed-version recovery are not certified by this audit. Existing
   compatibility docs and ADRs describe these boundaries.

No runtime/API/schema changes or new tests are needed for this verified no-op.
No new differential or benchmark result is claimed. The October 22 pinned
v0.18.0 differential scenarios remain historical evidence only; this audit
rechecks source and reruns the existing Rust regression coverage.

## Validation and delivery

The baseline's actual main-SHA workflows passed:
[Check](https://github.com/leo91000/graphile_worker_rs/actions/runs/40314830798),
[Test](https://github.com/leo91000/graphile_worker_rs/actions/runs/40314830753),
[Coverage](https://github.com/leo91000/graphile_worker_rs/actions/runs/40314830987),
[Docs](https://github.com/leo91000/graphile_worker_rs/actions/runs/40314830660).
These are baseline evidence, not substitutes for this report PR's final-head
checks and actual review decisions.

Local validation uses Rust 1.99.1 and disposable PostgreSQL 18.6. The image
resolved to `postgres@sha256:979bccfde35445db618c65107d27d08c6c21339d8dc3ad4c830c6633947be0e4`.

- `just lint` passed: format, all-target check and Clippy with warnings denied.
- The fresh complete `just test-docker` recipe passed: **524 passed, zero failed,
  six existing ignored tests**, including documentation tests. It used four
  build jobs and one test thread; the fixture container was removed by the recipe.
- All 26 run-once tests, five SQL retry integrations, five upstream-sync tests,
  five retirement/recovery tests, 12 migration tests, 21 cleanup tests and the
  queue-name regression passed within that complete run. Existing assertions,
  deadlines and supported features remain intact.
- mdBook 0.4.52 built successfully; all 14 Python release-tool regressions and
  six TLS dependency-isolation checks passed.
- Locked UI dependencies were installed with development dependencies included.
  The source-map offset/valid-map regression passed, also with CI-pinned Node
  24.22.0. Fresh npm audit reports zero alerts; no manifest/lockfile change was
  needed to close the withdrawn braces advisory.
- Inventory verification found all 147 full commit SHAs in the cumulative
  individual ledgers. Migration hashes and the empty migration diff confirm
  revisions 1–22 remain unchanged since the last audit's baseline.

Applicable final-head CI still must verify PostgreSQL 14–18, both TLS protocols,
four driver/TLS coverage variants, three OpenTelemetry feature configurations
and docs. Additional supported runtime validation proceeds alongside hosted
checks. The task checkpoint and durable final run output retain their exact
commands/results, head/merge SHAs, workflow IDs, actual review decisions and
pending gates. Baseline or previous-head success is not substituted for those
delivery gates. PR body is empty as required; this report is the nonempty
deliverable.

### Initial local timing failure

The first `just test-docker` attempt used two build jobs and two test threads.
All 56 root unit tests passed, then accepted_flags finished with five passed and
one failed. The continuous-routing matrix exceeded its unchanged five-second
progress deadline for the empty accepted set / forbidden GPU case: one eligible
job had executed when eight were expected. The suite took 363.75s. At inspection,
a concurrent isolated fixture's CREATE DATABASE was waiting on PostgreSQL
VersionFileSync. This is an observation of storage synchronization, not proof
of the precise stall cause. No production runtime/test source differs from the
baseline in this report-only change.

The unchanged focused matrix passed with one test thread: all 15 combinations
of filters and direct/one-local-queue/two-local-queue operation completed in
99.00s. The original assertions, five-second progress/shutdown deadlines and
PostgreSQL fsync/synchronous_commit settings remain intact. A fresh full required
recipe is then run with serialized test execution. The initial failed attempt
is retained separately and is not counted as a successful full suite.
