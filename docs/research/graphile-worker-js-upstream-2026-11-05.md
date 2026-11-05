# Graphile Worker upstream audit — 2026-11-05

## Result and inspection checkpoint

This audit identifies and ports the newly merged named-queue release wakeup fix
from upstream #653. The other new merge updates upstream PostgreSQL CI setup.
Published stable remains v0.18.0; the fix is **merged but unreleased**. This is
verified coverage of the ranges below, not a claim of full cross-runtime parity.

| Reference | Verified SHA / range |
| --- | --- |
| Rust main baseline | `4d536e95e509c13d79df96aee67f8cfd880ee31c` |
| Previous inspected upstream checkpoint | `7a2405bdd3b6322fb6b0d48dc0543bcc89166930` |
| Current inspected upstream main | `b4f1b41c7abd747586879bf68a46a5a22897fa96` |
| Incremental range | `7a2405bdd3b6322fb6b0d48dc0543bcc89166930..b4f1b41c7abd747586879bf68a46a5a22897fa96`: two first-parent merges, nine reachable commits |
| Published stable v0.18.0 | `4cda192c5df254392a1dff350e5d73f7d2c18a85`, published 2026-09-08 |
| Merged but unreleased | `4cda192c5df254392a1dff350e5d73f7d2c18a85..b4f1b41c7abd747586879bf68a46a5a22897fa96`: three first-parent merges, eleven reachable commits |
| Maintenance `0.17.x` / v0.17.3 | `ca2cef1c8c6bf9d9c818d19f194f596971c212df`; no commit outside main |
| Cumulative inspected range | v0.16.0 (`188c330e76e036bf2671e772c9b86b93beb5b31b`, exclusive) through current upstream main: 149 first-parent deltas |

Fetched live official refs/tags, releases/notes, CHANGELOG, merged and open PR
metadata, complete new merge diffs and source/tests. Stable-to-main changes are
not inferred from release objects: the diff includes queue completion/failure,
worker lifecycle, the backlog regression, npm depcheck, CI and yarn.lock.
Upstream is comparison-only; no upstream write was made.

No local checkpoint directory existed. Reconstructed the inspection checkpoint
from committed reports, fetched full upstream history and checked ancestry.
Every one of the prior 147 first-parent SHAs occurs in an individual ledger row:
41 in the [October 22 v0.16.0..v0.16.6 ledger](graphile-worker-js-upstream-2026-10-22.md#broadened-published-stable-ledger-v0160v0166),
105 in the [September 17 v0.16.6..v0.18.0 ledger](graphile-worker-js-upstream-2026-09-17.md),
and one in the [October 29 unreleased ledger](graphile-worker-js-upstream-2026-10-29.md#merged-but-unreleased-ledger).
Those rows remain part of this cumulative ledger, with the current-source
behavioral dispositions below superseding older deferrals. The two new merge
rows and their seven branch commits are individually recorded below. No
history divergence or uncovered commit was found in the reconstructed range.

The September 5 report is absent from main and its fetched file history. The
previous audit broadened to v0.16.0 for that reason. Pre-v0.16.0 behavior remains
outside certified coverage; extending coverage requires recovered verifiable
evidence or direct comparison of that older range. Advancing the inspected SHA
does not remove this limit or the proposal/compatibility conditions below.

Read AGENTS.md, current architecture, ADRs 0001–0005, comparison reports,
compatibility/runtime/migration/local-queue guidance, justfile, pinned Rust and
actual Check/Test/Coverage/Docs workflows. Used a clean isolated worktree.
No competing upstream synchronization PR was open; unrelated dependency/release
PRs were preserved. Main baseline workflows Check, Test, Coverage and Docs had
passed at the recorded exact SHA. No manual release/tag or production queue
operation was performed.

## Merged but unreleased ledger

| Upstream commit / PR | Changed behavior / affected components | Disposition and source evidence |
| --- | --- | --- |
| [7a2405bdd3b6322fb6b0d48dc0543bcc89166930 / #634](https://github.com/graphile/worker/commit/7a2405bdd3b6322fb6b0d48dc0543bcc89166930), branch `29b73dcc5dc817399f7dcb5c564f701639a3228d` | Joi/address security/dependency lock changes. | **Inapplicable to Rust.** Complete diff is yarn.lock only. Cargo and the independent admin UI graph do not execute these Node validators; Rust uses typed options. Retained from prior checkpoint, still unreleased. |
| [c2203f4d13ddadca1e88607c3c38245b2fa3769c / #652](https://github.com/graphile/worker/commit/c2203f4d13ddadca1e88607c3c38245b2fa3769c) | PostgreSQL CI apt setup: scoped GPG keyring, runner-codename repository, pg_dump client 14 replacing 11; move yarn install after setup. | **Inapplicable implementation; equivalent testing capability retained.** Rust Test/Coverage use PostgreSQL service containers and no apt-key, PGDG repository or dump executable. Existing migration fixture contains official SQL; no pg_dump import behavior changes. PG14–18 and both drivers/TLS protocols remain CI gates. |
| `88434d21408d4d9fccf4abcf7912f20bbe4ca314` (#652 branch) | Same PostgreSQL CI setup change. | **Inapplicable** for the concrete CI design reason above; contained in the complete #652 merge delta. |
| [b4f1b41c7abd747586879bf68a46a5a22897fa96 / #653](https://github.com/graphile/worker/commit/b4f1b41c7abd747586879bf68a46a5a22897fa96) | Post-persistence named-queue completion/failure nudges; LocalQueue pulses; shutdown guards; backlog regression; depcheck/changeset metadata. | **Implemented.** Rust `queue_wakeup.rs`, continuous sources, direct release and both batchers share bounded counted hints. Only persisted named outcomes wake fetchers. Tests cover serial backlog progress, direct/batched completion and retryable/permanent failure, one/two local queues with delayed refetch, shutdown, fallback and partial persistence. ADR 0006 records Rust adaptation. npm depcheck/changesets are inapplicable to Cargo. |
| `e2d84a6f916cc97fe179d3af3076df4ea5f7caaf` (#653 branch) | Initial queue-unlock wakeup approach and backlog tests. | **Implemented final behavior.** Final merge replaces parts of this intermediate worker continuation with post-persistence pool nudges. Rust ports the final source, preserving awaited direct errors and run-once continuations. |
| `1a1602380fb1ae10340f76fadae76f94573f3a5f` (#653 branch) | Patch changeset describing named-queue drain regression. | **Inapplicable packaging; documentation implemented.** Rust does not publish npm changesets. This report, ADR 0006 and local-queue guide explain the behavior. |
| `d875b7559f596036d16a0e2c35450777c97c9bf3` (#653 branch) | Central nudge active/shutdown guard. | **Implemented / Rust lifecycle.** Wakeup stream prioritizes shutdown and slots retain shutdown-first receive; late hints own no jobs and cannot restart a Released LocalQueue. Deterministic shutdown regression covers pending hints. |
| `359b600cc8a508afb901baaac88fa73e2484b7f1` (#653 branch) | Maintainer's final batch/direct SQL completion/failure nudges and local-queue pulse approach. | **Implemented.** Direct success/failure wake only after awaited SQL; asynchronous batch/fallback wake only successfully persisted named outcomes. Counted coalescing retains refetch-threshold contributions. No PostgreSQL notification per completion. |
| `4fc9ddc6a1878386bb1dbf81537221e850322057` (#653 branch) | Ignore @changesets/* in Node depcheck. | **Inapplicable.** Neither Cargo nor independent Rust UI checks run upstream depcheck/changesets. No queue/SQL behavior in this branch delta. |
| `147f45867ef80a8c43513ef03ad0a3b9c31bd053` (#653 branch) | Lint/import formatting of the new backlog test. | **Inapplicable syntax; equivalent regression implemented.** Rust tests use TaskHandler and isolated PostgreSQL with bounded progress rather than Jest/TypeScript imports. |

## Implemented Rust behavior and compatibility

Baseline reproduction on the recorded Rust main, using only the new test:
`completion_wakes_named_backlog` failed with **2 of 8** jobs executed before the
unchanged five-second progress bound, with a 60-second polling interval and no
notifications. Rust direct slots consume wakeup hints rather than automatically
continuing forever, so awaiting completion alone did not prevent this stall.

`QueueWakeup` now coalesces releases using one runtime notification and a
saturating count. The continuous source stream immediately polls direct slots,
or pulses all worker-owned LocalQueues before signalling their consumers.
Configured refetch-delay thresholds, filters, one-job-per-named-queue locking,
retry scheduling and pending-return claim coordination remain intact. Mere
batch acceptance does not wake a fetcher. Failed or partially persisted outcomes
are not counted as released; successful fallbacks are counted. Wakeup happens
before application observer hooks, so their latency does not delay the hint.

Shutdown wins over queued hints. No new detached task, global broadcast or
PostgreSQL notification is introduced. Other processes retain their polling and
insert-notification behavior. Existing public hook source variants are reused:
Polling for direct poll hints, LocalQueue for local pulses. Run-once retains its
own fetch-stop, in-flight claim-return and error-draining rules. There is no new
migration or SQL/public-option change. Applied migrations 1–22 and root/upstream
MIT attribution are retained; PostgreSQL minimum remains 12. See
[ADR 0006](../adr/0006-named-queue-release-wakeups.md).

## Published-stable behavior rechecked on current main

The earlier ports actually landed: [#524](https://github.com/leo91000/graphile_worker_rs/pull/524)
at `4d2e979`, [#547](https://github.com/leo91000/graphile_worker_rs/pull/547)
at `fd8411c`, [#568](https://github.com/leo91000/graphile_worker_rs/pull/568)
at `607b218`, and [#576](https://github.com/leo91000/graphile_worker_rs/pull/576)
at `f22bfd931b552e7805844c055736f9411a4b5e6c` are ancestors of the baseline.
The latter contains the October 22 report, SQL retries, queue-name helpers,
cleanup keep list, run-once fetch-stop/claim-return fixes and source-map fix.
The subsequent diff to this audit baseline contains only Cargo/example/documentation
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

All previous live heads were compared with the prior report; unchanged SHAs
retain the source/test comparison below. #651 and #654 are newly tracked and their full diffs were read.
Open proposals are not merged behavior.

| Proposal / inspected head | Behavior, affected components and current disposition |
| --- | --- |
| [#633](https://github.com/graphile/worker/pull/633), `13679fe51b7751597db8dc1ffebaf8a69f58e2cb` | **Open; track.** Explicit migration entrypoints/error cleanup. WorkerUtils construction is migration-free, migrate is explicit, WorkerOptions init migrates before registration. Owned Rust handles/Results replace JS releaser arrays. No newly demonstrated missing behavior. |
| [#637](https://github.com/graphile/worker/pull/637), `3925e59a44d9f0d2d3fda2445053183d4c093ed3` | **Open; track.** Rejected/stale task-lookup cache. Rust fetches fresh Results; init shares successful details only. Cleanup refresh compares snapshots under its write lock and returns a bounded conflict rather than overwriting concurrent registrations. Existing cache/race tests remain. |
| [#640](https://github.com/graphile/worker/pull/640), `74ccbfd8c8780e5ca9b81ed3ee5924f8c87e0375` | **Closed unmerged.** Alternate rejected-cache proposal. Same Rust comparison as #637; closure is not a merged implementation. |
| [#642](https://github.com/graphile/worker/pull/642), `6c6b57897d303db3713e9b9bf3c21d4e3666f725` | **Open; inapplicable.** Upstream sponsor list only; does not establish downstream sponsorship or queue behavior. |
| [#643](https://github.com/graphile/worker/pull/643), `11201b5e52a64f533b62a99c1d01bfdab8e608cc` | **Open; track.** Unbatched outcome-error reporting. Rust awaits direct completion/failure, preserves job ID/SQLSTATE/diagnostic and drains active jobs before returning run-once errors. Closed-channel fallback errors propagate. Existing persistence fault regressions remain. Asynchronous batch-error policy still differs. |
| [#644](https://github.com/graphile/worker/pull/644), `7db0b5ff0cd36d2283b65e0eaee20bbad695a9be` | **Open; track.** Keyed scheduling serialization. Rust revision 22 retains sorted advisory locks from revision 19, locks existing rows and keeps GWNOR for missing results; keyed contention regression checks returned jobs and one surviving key. The advisory namespaces differ; mixed Node/Rust producer coordination is not certified. |
| [#645](https://github.com/graphile/worker/pull/645), `0a77c0704669a0a06adda3c5ff504c8507889e58` | **Open; track with differences.** Node batch-loop retries and backpressure-deferred reset. Rust bounded channels have no matching deferred/refcount protocol. Optional pooled SQL retries provide a separate policy, while async completion logging/failure per-job fallback remains distinct from fatal-pool shutdown. Reassess final policy/fault cases if merged. |
| [#646](https://github.com/graphile/worker/pull/646), `45b79c00f812298d6bd838d17563d6c7f44aac3d` | **Open; unchanged head, track with compatibility decision.** Keyed insert/claim race, array retry exemption, in-place updates of unlocked exhausted keyed jobs, deletion of superseded scalar jobs in force-unlock/stale-reset/return paths and corresponding docs/tests. Rust's ordered key and row locks already address its keyed race. Revision-22 retirement markers deliberately retain retired rows, release ownership without restoring attempts and allow explicit rescheduling. Like stable upstream, its add_jobs retires unavailable keyed rows, including exhausted unlocked rows, rather than adopting this proposal's locked-only retirement condition. Markers distinguish ordinary final attempts from retirement rather than inferring retirement from key/payload/error fields. The proposal's changed retirement/deletion rules are absent from stable/main and are not imported into applied SQL. Re-evaluate final merged SQL, partial array failures, both recovery paths, upgrade history and cross-runtime interaction when it lands (ADRs 0002/0005). |
| [#647](https://github.com/graphile/worker/pull/647), `1bf3da11c43711975f9cfd4f75a53f2b43df0c32` | **Open; dependency-only tracking.** follow-redirects, handlebars, http-cache-semantics, Joi, shell-quote and undici changes do not occur in the Rust worker or independent UI build graph. The overlapping source-map-js fix already landed at 1.2.2 in #576; its oversized-offset/valid-map regression is in Check CI and passed locally. This proposal still is not a merged queue port. |
| [#648](https://github.com/graphile/worker/pull/648), `91ef5723e730b48de6b563b04cd2d1800e6c64b2` | **Open; track; independent Rust correction already landed.** Runner/CLI error propagation. Continuous Rust run preserves the original source error through draining/deregistration. #576 retains run-once fetch errors, stops stream and named continuations, and awaits/returns committed unstarted claims. `tests/run_once/fetch_errors.rs` includes initial/stream/follow-up failures, both in-flight directions and rejected-return SQLSTATE injection. Original error and claim ownership remain observable. |
| [#649](https://github.com/graphile/worker/pull/649), `9b3b5968d77d715cea2b4d2d7a6237d2f6fcdf5d` | **Open; unchanged documentation proposal.** Dedicated local-queue options, tuning, tradeoffs and events page. Rust already has `guides/local-queue.md`, worker options/queue guides and typed hooks. Compared with actual `local_queue/config` and `refetch_delay.rs`: Rust enables it through an Option, uses size 100 per queue, adds queue_count, and jitters refetch duration by 0.5–1.0 rather than Node's 0.5–1.5. Rust's documented return/recovery rules differ from Node's four-hour/Pro copy. Keep Rust-specific docs; do not copy incompatible defaults or claim the proposal is merged. |

| [#651](https://github.com/graphile/worker/pull/651), `2fe12be43451f1e2e607c878730dbffa99f95911` | **Closed unmerged; CI-only proposal.** Pins upstream database dump CI to Ubuntu 24.04 to retain apt-key. Complete diff changes one runs-on value. Rust uses PostgreSQL service containers without apt-key or that dump job; the official merged alternative #652 is separately accounted for. No queue behavior or Rust runner pin change is required. |
| [#654](https://github.com/graphile/worker/pull/654), `4dc9497354ec92e95546f9dbcdb07c4fa0283681` | **Open; release metadata only.** Proposes npm 0.18.1, a version constant, changelog entry and removal of the consumed changeset. It contains no additional queue fix. #653 is already merged and audited below, but 0.18.1 is not a published stable release at this checkpoint. Manual releases are outside this task. |

## Differential and regression evidence

A disposable PostgreSQL 18.6 service hosted separate UUID databases for every
scenario. Node 24.22.0 loaded the published `graphile-worker@0.18.0` package and
a source snapshot exported from exact upstream
`b4f1b41c7abd747586879bf68a46a5a22897fa96`, with SQL generated by that snapshot's
own build script. Its dependency resolution was captured in the fixture lockfile.
No source or remote branch in the comparison-only upstream clone was modified.

Each fixture inserts eight due jobs into one named queue before starting three
slots with a 60-second poll interval. The Node matrix varies LocalQueue off/on,
direct/10ms-batched outcomes, and success/first-job permanent failure. It asserts
serial execution, consumed attempts, persisted failure diagnostics, unlocked
ownership and the final database row count. The 1.5-second progress budget starts
after startup; reported wall times include initialization/migration and shutdown.
Stable v0.18.0 stalls in all eight scenarios with only 1–3 of eight executions.
The pinned upstream merge completes all eight scenarios with eight executions
and the expected persisted state. These measurements reproduce the defect and
fix; fixture setup time is not a throughput benchmark.

The Rust tests use the equivalent eight-job/three-slot isolated workload,
explicitly disable notifications, and add a second LocalQueue mode. LocalQueue
refetch is deliberately delayed for 60 seconds with abort threshold one, so
normal timer expiry cannot explain progress. Six tests cover 18 mode/outcome
combinations: direct/batched completion, retryable failure and permanent failure.
Retryable fixtures begin with three consumed attempts so the next retry's
backoff exceeds the five-second progress bound even on a slow node. Wakeups must
not consume another attempt or bypass that backoff. Unit/fault tests additionally
cover early/coalesced/saturating hint counts, stale permits, shutdown priority,
closed-channel completion/failure fallbacks, rejected named completion alongside
a successful unnamed completion group, and partially persisted failure batches.

The Node harness first exposed its own cleanup/observation races: a library-owned
pool was still disconnecting when a test database was force-dropped, and handler
counts were observed before fire-and-forget persistence finished. The corrected
harness owns/awaits its pools, waits for database outcomes and disconnected
sessions, and completes all 16 scenarios successfully. Earlier incomplete runs
are retained separately and are not counted as final validation. Rust's own
fixture cleanup and normal PostgreSQL durability settings remain unchanged.

The manual Rust benchmark uses 200 serial jobs with a 1ms handler await,
three slots and no notifications, with direct/10ms-batched outcomes and
zero/one/two LocalQueues. It preserves the regression's final-state and serial
execution assertions. Measured results are recorded after it completes; the
30-second outer bound detects a stalled drain; no universal throughput target
is enforced.

## Revisited unresolved items and limits

1. **No identified applicable merged port in the inspected 149-delta range is
   intentionally deferred.** Delivery validation/review remains a separate gate
   from the inspection checkpoint; an inspected SHA is not implementation proof.
2. **Pre-v0.16.0 coverage remains uncertified.** The missing September 5 report
   could not be recovered from fetched history. Directly audit that earlier
   range or recover verifiable evidence to extend this boundary.
3. **#645 batch error policy remains a tracked proposal.** Rust opt-in pooled
   SQL retries and asynchronous log/per-job fallback differ from Node fatal-pool
   semantics. Reassess the final source, retry/backpressure and fault cases when
   upstream merges it; do not treat the current proposal as required behavior.
4. **#646 keyed/array retirement remains a tracked compatibility decision.**
   Rust ordered key locks and revision-22 retirement markers are retained. On
   upstream merge, compare final SQL, partial-array failures, reschedule/recovery
   races and upgrade paths. The current proposal is not imported into applied SQL.
5. **Other proposals keep the individual dispositions and merge triggers above.**
   #654 proposes 0.18.1; it does not make the fix published stable. All prior
   proposal heads are unchanged from October 29, including closed-unmerged #640. Newly closed-unmerged #651 is also tracked.
6. **Braces is still closed as a withdrawn advisory, not a patch.** The prior
   deferral was resolved by withdrawal of GHSA-vfj7-8cjw-p6xm, not a package
   downgrade. The independent source-map-js 1.2.2 fix/regression remains retained.
   Fresh local UI audit results are recorded in validation below.
7. **Documented intentional differences remain.** SQL retries are opt-in;
   caller-owned transactions are not replayed; queue-name misses use Option;
   observer isolation excludes aborting panics; local-calendar cron is deliberate;
   Node return SQL does not consult Rust retirement markers. Equal migration
   numbers, different advisory-lock namespaces, every proxy/OS/PG12 combination
   and mixed-version recovery are not certified by this task. Existing ADRs and
   compatibility guidance retain the operational boundaries.

8. **Separate requirements/capabilities model remains a product design proposal
   (ADR 0004), not missing merged upstream behavior.** Upstream still uses job
   flags for routing/operational labels and has no merged separate capability
   taxonomy. Rust's accepted/forbidden filters and optional allowlist requiring all flag keys
   remain supported and tested, including operational flags. A separate model
   requires an explicit taxonomy, public protocol and worker rollout
   contract, or a final merged upstream design to compare. No such contract can
   be inferred from #653's wakeup change; no existing filtering capability is
   removed or silently reclassified.
9. **Historical retirement events before Rust revision 22 cannot be inferred.**
   Existing rows lack the replacement/removal markers that distinguish retired
   jobs from legitimate final attempts. ADR 0002 deliberately leaves those rows
   unmarked; guessing from exhausted attempts or a missing key would change
   legitimate recovery behavior. Extending protection retroactively requires
   trustworthy historical event evidence or an explicit administrative decision
   for known jobs. No production data operation is performed, and the applied
   migration history remains unchanged. Current replacement/return/reschedule
   race tests verify the path using retirement markers, not recovery of lost history.

## Validation and delivery

Validation results are appended after commands complete. Applicable final-head
Check, Test (PG14–18 and TLS1.2/1.3), four driver/TLS Coverage variants, Codecov,
Docs and actual review decisions must pass before merge. Main CI at the actual
merge SHA remains a separate required gate. PR body is empty under the selected
maintenance-delivery skill; this committed report contains the detailed evidence.

### Completed tooling checks

- Final `just lint` passed: format, all-target check and Clippy with warnings
  denied, using pinned Rust 1.99.1 and at most three build jobs.
- mdBook 0.4.52 built the updated documentation successfully.
- All 14 release-tool Python regressions and six TLS dependency-isolation
  checks passed. Locked UI development dependencies were installed with scripts
  disabled; the source-map oversized-offset/valid-map regression passed with
  CI-pinned Node 24.22.0. Fresh npm audit reports zero vulnerabilities.
- The 149-SHA individual-ledger check passed. Migrations 1–22 have an empty
  diff to the main baseline; SQL hashes are retained in run output.

Local database validation uses PostgreSQL 18.6, image
`postgres@sha256:3a4264520e121176619f54ff867d12774bc81e95e7b84db58d63bccdb6144b81`.
Full required and runtime suites are recorded after their completion below;
results from baseline or intermediate implementations do not replace final CI.

### Interrupted local execution

The first complete `just test-docker` attempt was interrupted by an execution
node restart after all 60 root unit tests and the first accepted-flags scenario
passed. Neither a final suite result nor Docker cleanup completed. The separate
tokio-postgres compilation was also interrupted. These attempts are retained as
partial evidence, not counted as successful full suites. On resume, process and
container state, saved files, main and the remote branch/PR inventory were checked
before restarting the isolated Docker recipe. No commit, push or PR had occurred.

### Required complete suite after recovery

The resumed full `just test-docker` recipe passed with **534 passed, zero failed
and seven ignored tests** across 89 test/doc-test suites. Six ignores predate
this port; the additional ignore is the explicitly invoked manual drain benchmark.
All 60 root unit tests and six new backlog regressions passed, including the
18 direct/one-local-queue/two-local-queue outcome scenarios. Existing run-once,
SQL retry, concurrent claim/identity, retirement/recovery, migration/dual-revision-20
upgrade, cleanup and queue-name assertions passed. The recipe used at most three
build jobs and one test thread, retained normal PostgreSQL durability settings,
and removed its disposable container successfully.

### Manual drain measurements

The explicitly invoked `benchmark_named_queue_drain` passed all six scenarios
(**1,200 total jobs**), with no serial-execution, persistence or queue-unlock
assertion failure. It ran the just-built default-suite executable directly while
Cargo compiled the independent runtime variant; this avoids a second Cargo build
lock, not any assertion. Reproduce with
`cargo test --test queue_wakeup benchmark_named_queue_drain -- --ignored --exact --nocapture --test-threads=1`
and an isolated DATABASE_URL. Each measurement starts after preloading its 200 jobs.

| Outcomes | Direct | One LocalQueue | Two LocalQueues |
| --- | ---: | ---: | ---: |
| Unbatched | 4,345 ms | 28,220 ms | 7,215 ms |
| 10ms batching | 10,923 ms | 16,870 ms | 11,165 ms |

The node has three available CPUs and concurrent compilation used one build job.
Database creation and sync latency varied during validation. These are bounded
correctness/drain measurements for a 1ms handler and one named queue, not evidence
of a universal speedup or a LocalQueue tuning recommendation. All configurations
drain before the 60-second poll/refetch timers, retaining the same 30-second
outer stall bound and final-state assertions.

### Additional runtime gates

The complete tokio-postgres/Tokio and SQLx/async-std test matrices were compiled
successfully and are executed sequentially on separate UUID fixtures. Their
final results, PR/head/merge SHAs, actual review decisions and all workflow IDs
are retained in the task checkpoint and durable final run output. They must
complete before delivery; successful default tests or old CI do not substitute
for those results. No supported feature/driver assertion is removed.


### Review follow-up

The review at `aca8e551aaee3ce11ac9faf19c3c83469139fa10` returned
**CHANGES_REQUESTED**, despite its walkthrough describing minimal merge risk.
Its one inline finding is valid: the counter update closure always produces a
new value, so `AtomicUsize::update` removes an unnecessary Result/expect path.
The pinned Rust 1.99.1 supports this API. The follow-up retains AcqRel/Acquire
orderings, saturating addition, notification placement and all assertions.
Existing coalescing, saturation, stale-permit and shutdown regressions verify
the unchanged behavior. Required validation is repeated before committing.

Initial Codecov reports all modified coverable lines tested and 97.5% project
coverage. These results belong to the initial head; the follow-up needs fresh
CI. The included review allowance is one per hour, exhausted after the review
submitted at 2026-11-05T10:18:58Z. A full re-review must be explicitly requested
after the provider's reset if it does not resume. Resolved threads, replies or
a successful review status do not supersede that blocking decision. The
inconclusive description suggestion is inapplicable because the selected skill
requires an empty PR body; the detailed evidence is committed in this report.


The initial head's actual Check, Test (PG14–18 and TLS1.2/TLS1.3), Coverage,
Docs, codecov/patch and codecov/project gates all passed. Its full local
Tokio/tokio-postgres matrix passed **296 tests, zero failed, three ignored**.
These successes do not clear CHANGES_REQUESTED or validate the follow-up head;
all applicable hosted gates and the actual review decision are checked again.

The complete SQLx/async-std matrix then passed on the atomic follow-up source:
**532 passed, zero failed, seven ignored**, including all six named-backlog
regressions. Database tests remained sequential and used isolated UUID fixtures.
The broader Tokio/tokio-postgres pass above remains initial-head evidence;
the changed counter and named-backlog regressions are rerun specifically for
that driver on the follow-up, in addition to fresh required default checks.


The follow-up passed `just lint` and the complete required `just test-docker`
recipe again: **534 passed, zero failed, seven ignored**, with successful
container cleanup. The fresh Tokio/tokio-postgres checks passed both counter/
shutdown unit regressions and all six backlog tests (18 queue-mode scenarios;
the manual benchmark remains explicitly ignored in this regression command).
Together with the complete 532-test SQLx/async-std run on the updated source,
these validate the atomic cleanup without weakening assertions or runtime
coverage. Fresh-head hosted CI and actual re-review are still required.


### Mixed-backlog review finding and direct continuation

The explicitly requested full re-review at 2026-11-05T11:35:20Z returned
**CHANGES_REQUESTED** on `97a73c8e37b52a776658be59a21c4e772e0c8b17`.
Its new finding is valid: a one-shot direct fanout can be consumed entirely by
higher-priority unnamed jobs after a named queue releases, leaving the remaining
named backlog waiting for a poll. This is separate from the atomic cleanup.

A gated regression reproduces the defect before the fix. The first named
handler holds its claim while twelve higher-priority unnamed jobs are inserted,
with two more named jobs already pending, no notifications and a 60-second poll.
At concurrency one, the initial PR head completes exactly **one named and one
unnamed job**, then exceeds the unchanged five-second progress bound. The gate
establishes insertion/claim order without timing sleeps.

Direct slots now continue on each successful `Some(job)` fetch outcome until
`None` reports that no ready work remains. A named release hint can therefore
start a drain that passes through the finite unnamed prefix rather than being
spent on one fetch per slot. This is the same continue-until-empty model as
Node's ordinary worker loop. Subsequent polls use the existing Polling hook
source. Each additional claim is preceded by polling the shutdown signal;
in-flight claim/handler/persistence is still awaited, avoiding uncertain
commit cancellation. SQL ownership, filters, priority, retries, batching,
LocalQueue pulses and run-once behavior remain unchanged. ADR 0006 and the
architecture/queue guide document the stronger continuation contract.

The regression varies concurrency one/three and direct/10ms-batched completion.
It checks all fifteen outcomes persisted, no overlap of named handlers and
released queue locks before any normal poll. A second gated case requests and
observes shutdown before releasing the first handler; all fourteen unstarted
jobs must retain zero attempts and unlocked ownership. It covers batched and
unbatched outcomes. Required checks and affected runtime/driver coverage are
repeated for this functional follow-up; previous heads' results remain separate.

A second pinned Node differential reproduces the same mixed fixture at
concurrency one/three with direct/10ms-batched completion. Each UUID database
starts with three named jobs; the gated first handler pauses while twelve
higher-priority unnamed jobs are inserted. The caller-owned pg pool suppresses
its clients' `notification` events to match Rust's disabled insert-notification
delivery, without editing vendor source or suppressing post-persistence pool
nudges. All eight fixtures retain serial-named-handler assertions and wait for
persisted rows/queue locks; pools and database sessions are awaited before cleanup.

Published v0.18.0 completes twelve unnamed and two named jobs in all four cases,
leaving one named job pending after the five-second progress budget. Pinned
upstream `b4f1b41c7abd747586879bf68a46a5a22897fa96` completes all fifteen jobs,
with zero rows and released queue locks in every case (384–878ms wall time,
including setup). The corrected Rust regressions likewise complete all fifteen
outcomes. This confirms that direct continuation plus the post-persistence
release hint is needed; additional insert notifications cannot mask the test.


The first functional-follow-up required run passed lint and all 60 root unit
tests, then failed the existing continuous accepted-flags routing matrix at
its five-second **shutdown** bound (five other accepted-flags tests passed).
The unchanged focused matrix subsequently passed all fifteen direct/one-local/
two-local filter combinations in 29.40s without concurrent compilation. This
resembles an earlier documented VM timing failure, but does not establish the
precise cause. The failed log is retained separately. A fresh full required
recipe uses the unchanged assertions, deadlines and PostgreSQL durability
settings, with serialized database tests and no concurrent compilation.

The fresh functional-follow-up `just test-docker` recipe passed **536 tests,
zero failed, seven ignored**, across 89 suites, including all eight wakeup
regressions. Its fixture container was successfully removed. `just lint` and
the pinned mdBook 0.4.52 build also passed. Alternate runtime/driver runs and
the manual 1,200-job drain measurement follow sequentially, preserving the
failed attempt and all previous-head results as separate evidence.

The functional follow-up's full SQLx/async-std matrix passed **534 tests, zero
failed, seven ignored**, across 89 suites. This includes all mixed/direct
continuation, shutdown, recovery, cleanup and migration assertions on the new
source; earlier atomic-only runtime results are not substituted for it.

The first full Tokio/tokio-postgres attempt passed all 53 applicable root tests,
then failed the existing accepted-flags matrix's unchanged five-second progress
bound: four of nine eligible jobs executed and the remaining five rows were
unclaimed. The unchanged focused fifteen-case matrix passed in 36.34s. Slow
statement logging on the disposable server observed COMMIT/fixture operations
above 500ms in that passing run; this is storage-latency evidence, not proof of
the exact failure cause. No source, deadline, assertion or database durability
setting was relaxed. The failed attempt is retained separately; the complete
driver suite is repeated without compilation, with slow statements retained.

The fresh full Tokio/tokio-postgres matrix passed **298 tests, zero failed,
three ignored**, across 37 suites, including all eight wakeup regressions.
The repeated manual benchmark also passed all six 200-job configurations
(**1,200 persisted jobs**, serial named handlers, no locked queue left behind).
Without concurrent compilation, direct/one-local/two-local queues took
3,549/4,135/4,464ms unbatched and 7,392/10,991/7,736ms with 10ms completion
batching. All are below the unchanged 30-second stall bound. These are
representative end-to-end VM measurements with a 1ms handler await and variable
storage synchronization, not a controlled universal throughput comparison.
Fresh-head hosted workflows and an actual nonblocking review decision remain
required before guarded merge; final run output records their exact SHAs and
results rather than attributing earlier-head checks to this functional fix.
