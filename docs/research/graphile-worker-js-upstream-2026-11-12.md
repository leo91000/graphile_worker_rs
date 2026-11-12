# Graphile Worker upstream audit — 2026-11-12

## Result and inspection checkpoint

Verified implementation no-op: there are **zero new merged upstream commits**
since the November 5 checkpoint. Published stable remains **v0.18.0**; the
named-queue wakeup fix #653 remains **merged but unreleased**. Rust's port and
its review follow-ups actually landed in #581 and remain in current main.
This report records fresh verification and the new open task-file-path proposal
#655; it does not claim full cross-runtime parity.

| Reference | Verified SHA / range |
| --- | --- |
| Rust main baseline | `81520d5f31ad2a7a821674f995631dd0fbff4268` |
| Previous and current inspected upstream main | `b4f1b41c7abd747586879bf68a46a5a22897fa96` |
| Incremental inspected range | `b4f1b41c7abd747586879bf68a46a5a22897fa96..b4f1b41c7abd747586879bf68a46a5a22897fa96`: zero commits |
| Published stable v0.18.0 | `4cda192c5df254392a1dff350e5d73f7d2c18a85`, published 2026-09-08 |
| Stable-to-main, merged but unreleased | `4cda192c5df254392a1dff350e5d73f7d2c18a85..b4f1b41c7abd747586879bf68a46a5a22897fa96`: three first-parent merges, eleven reachable commits |
| Maintenance `0.17.x` / peeled v0.17.3 | `ca2cef1c8c6bf9d9c818d19f194f596971c212df`; zero commits outside main |
| Reconstructed cumulative range | peeled v0.16.0 `188c330e76e036bf2671e772c9b86b93beb5b31b` (exclusive) through current main: 149 first-parent deltas |
| Stable interval boundary | peeled v0.16.6 `5650fbc4406fa3ce197b2ab582e08fd20974e50c` |

Fetched full official upstream history, live branch/tag refs, all release
objects/notes, all merged/open PR metadata, CHANGELOG, the complete stable-to-main
diff, and current source/tests. Release objects alone are not the change inventory.
All twelve open PRs are individually tracked below. #648/#649 have newer activity
but unchanged inspected heads; #655 is new. No upstream writes were made.

The local state directory was absent. Reconstructed it from committed reports,
checked ancestry, and mechanically matched every full first-parent SHA to an
individual ledger row: 41 in the [October 22 ledger](graphile-worker-js-upstream-2026-10-22.md#broadened-published-stable-ledger-v0160v0166),
105 in the [September 17 ledger](graphile-worker-js-upstream-2026-09-17.md#change-ledger),
one in the [October 29 ledger](graphile-worker-js-upstream-2026-10-29.md#merged-but-unreleased-ledger),
and two in the [November 5 ledger](graphile-worker-js-upstream-2026-11-05.md#merged-but-unreleased-ledger).
Those individual rows remain part of this cumulative ledger; current-source
rechecks below supersede old proposed/deferred delivery statuses. All 149 are
covered, with no divergence or uncovered commit within the reconstructed range.

The September 5 report is still absent from main and fetched file history.
Pre-v0.16.0 coverage remains uncertified. This unchanged boundary is retained
separately from both the inspection checkpoint and implementation coverage;
recovering the report or directly comparing older history is required to extend it.

Read AGENTS.md, architecture, ADRs 0001–0006, all existing comparison reports,
compatibility, runtime/driver, migrations and local-queue guidance, justfile,
Rust pin 1.99.1, and actual Check/Test/Coverage/Docs workflows. A clean isolated
worktree preserves the shared checkout. No competing upstream maintenance PR
was open; three unrelated dependency PRs were left untouched. Baseline Check,
Test and Coverage passed at the exact recorded main SHA (runs 42898051472,
42898051356 and 42898051412); Release-plz also passed at 42898051352.
Docs was not triggered by that release-only baseline, so no baseline Docs result
is attributed to it. Manual release/tag and production queue operations are outside scope.

## Merged but unreleased ledger

The complete diff was reread; the eleven commits below remain unreleased.

| Upstream commit / PR | Changed behavior / affected components | Disposition and source evidence |
| --- | --- | --- |
| [7a2405bdd3b6322fb6b0d48dc0543bcc89166930 / #634](https://github.com/graphile/worker/commit/7a2405bdd3b6322fb6b0d48dc0543bcc89166930), branch `29b73dcc5dc817399f7dcb5c564f701639a3228d` | Joi/address security/dependency lock changes. | **Inapplicable to Rust.** Complete diff is yarn.lock only. Cargo and the independent admin UI graph do not execute these Node validators; Rust uses typed options. Retained from prior checkpoint, still unreleased. |
| [c2203f4d13ddadca1e88607c3c38245b2fa3769c / #652](https://github.com/graphile/worker/commit/c2203f4d13ddadca1e88607c3c38245b2fa3769c) | PostgreSQL CI apt setup: scoped GPG keyring, runner-codename repository, pg_dump client 14 replacing 11; move yarn install after setup. | **Inapplicable implementation; equivalent testing capability retained.** Rust Test/Coverage use PostgreSQL service containers and no apt-key, PGDG repository or dump executable. Existing migration fixture contains official SQL; no pg_dump import behavior changes. PG14–18 and both drivers/TLS protocols remain CI gates. |
| `88434d21408d4d9fccf4abcf7912f20bbe4ca314` (#652 branch) | Same PostgreSQL CI setup change. | **Inapplicable** for the concrete CI design reason above; contained in the complete #652 merge delta. |
| [b4f1b41c7abd747586879bf68a46a5a22897fa96 / #653](https://github.com/graphile/worker/commit/b4f1b41c7abd747586879bf68a46a5a22897fa96) | Post-persistence named-queue completion/failure nudges; LocalQueue pulses; shutdown guards; backlog regression; depcheck/changeset metadata. | **Already supported (landed #581).** Rust `queue_wakeup.rs`, continuous sources, direct release and both batchers share bounded counted hints. Only persisted named outcomes wake fetchers. Tests cover serial backlog progress, direct/batched completion and retryable/permanent failure, one/two local queues with delayed refetch, shutdown, fallback and partial persistence. ADR 0006 records Rust adaptation. npm depcheck/changesets are inapplicable to Cargo. |
| `e2d84a6f916cc97fe179d3af3076df4ea5f7caaf` (#653 branch) | Initial queue-unlock wakeup approach and backlog tests. | **Implemented final behavior.** Final merge replaces parts of this intermediate worker continuation with post-persistence pool nudges. Rust ports the final source, preserving awaited direct errors and run-once continuations. |
| `1a1602380fb1ae10340f76fadae76f94573f3a5f` (#653 branch) | Patch changeset describing named-queue drain regression. | **Inapplicable packaging; documentation implemented.** Rust does not publish npm changesets. This report, ADR 0006 and local-queue guide explain the behavior. |
| `d875b7559f596036d16a0e2c35450777c97c9bf3` (#653 branch) | Central nudge active/shutdown guard. | **Already supported / Rust lifecycle.** Wakeup stream prioritizes shutdown and slots retain shutdown-first receive; late hints own no jobs and cannot restart a Released LocalQueue. Deterministic shutdown regression covers pending hints. |
| `359b600cc8a508afb901baaac88fa73e2484b7f1` (#653 branch) | Maintainer's final batch/direct SQL completion/failure nudges and local-queue pulse approach. | **Already supported (landed #581).** Direct success/failure wake only after awaited SQL; asynchronous batch/fallback wake only successfully persisted named outcomes. Counted coalescing retains refetch-threshold contributions. No PostgreSQL notification per completion. |
| `4fc9ddc6a1878386bb1dbf81537221e850322057` (#653 branch) | Ignore @changesets/* in Node depcheck. | **Inapplicable.** Neither Cargo nor independent Rust UI checks run upstream depcheck/changesets. No queue/SQL behavior in this branch delta. |
| `147f45867ef80a8c43513ef03ad0a3b9c31bd053` (#653 branch) | Lint/import formatting of the new backlog test. | **Inapplicable syntax; equivalent regression implemented.** Rust tests use TaskHandler and isolated PostgreSQL with bounded progress rather than Jest/TypeScript imports. |


## Published-stable behavior rechecked on current Rust main

Ancestry checks confirm these delivered ports: #524 at
`4d2e979eb50fb292b9ab10eaf101edfca2546d13`, #547 at
`fd8411cfad9e5919fe6c28550b1d0c662aa638a8`, #568 at
`607b218c763e319f9b3609171f735db137e0dc48`, #576 at
`f22bfd931b552e7805844c055736f9411a4b5e6c`, and #581 at
`9fb97f3e356b810d86491cecc4a18264efd61eab`. Current source was inspected rather
than inferring parity from those PR names. All wakeup implementation/test files
are unchanged since #581, including AtomicUsize::update, continue-until-empty
direct slots, shutdown checks, mixed-backlog tests and post-persistence hints.

The subsequent non-release change #582 at
`a6648b332f5b627ae916d6f111604d4f46a9dc26` preserves native PostgreSQL error
sources using an Arc-backed source chain. Its full source diff retains SQLSTATE,
server diagnostics and acquisition-only retry classification. Driver-contract
and error_sources regressions cover cloned errors, pool/TLS/URL failures and
structured diagnostics. Release #563 changes crate versions/changelogs only.
The retry, migration, cleanup, cron and named-queue contracts below remain present.

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


The other individual cumulative ledger rows retain their concrete runtime/design
reasons: compiled TaskHandler registration has no ESM/CommonJS/TypeScript or
child-process task loader; typed builders have no cosmiconfig/graphile.config
runtime; tracing/typed hooks and awaited shutdown replace Node logger,
EventEmitter and ECMAScript disposal contracts. npm/yarn/Jest/website/publishing
changes affect tools and copy absent from Rust. No migration number or similarly
named API is used as proof of behavioral equivalence. Applied SQL 1–22 and MIT
attribution are unchanged. No new performance claim is made for cached SQL or
queue-name lookup.

## Open and closed proposals: tracking only

The live heads below were compared with the previous checkpoint. All previously
tracked heads are unchanged; their prior source/test comparisons and current
Rust evidence were revisited. #648 and #649 full diffs were reread despite only
metadata activity. #655's complete diff was newly inspected. Open PRs and
synthetic merge refs are not merged behavior.

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
| [#654](https://github.com/graphile/worker/pull/654), `4dc9497354ec92e95546f9dbcdb07c4fa0283681` | **Open; release metadata only.** Proposes npm 0.18.1, a version constant, changelog entry and removal of the consumed changeset. It contains no additional queue fix. #653 is already merged and audited in the unreleased ledger above, but 0.18.1 is not a published stable release at this checkpoint. Manual releases are outside this task. |
| [#655](https://github.com/graphile/worker/pull/655), `e805e3e6155e1c081781717ce6658447f68f2e3f` | **Open; inapplicable to Rust's task runtime.** Converts native dynamic import(filename) to import(pathToFileURL(filename).href), preventing `#`, `%`, `%20` and parent-directory characters from changing module selection. CJS/ESM tests spawn actual Node to avoid Jest resolver masking. Rust `src/builder` registers compiled TaskHandler functions and static identifiers; neither that path nor the administration CLI resolves dynamic JavaScript task-file URLs. No corresponding file import or URL decoding exists to patch. This is a meaningful Node loader correctness/security boundary, not merely packaging. Reassess if upstream merges it or Rust adds such a loader; no Node runtime is introduced. |

## Revisited unresolved items and compatibility decisions

1. **No identified applicable merged port is deferred within the 149-delta
   inspected range.** There is no new Rust implementation in this audit.
   Inspection and delivery checkpoints remain separate.
2. **Pre-v0.16.0 coverage remains uncertified.** The attempted file-history
   recovery above found no September 5 report. Recover verifiable evidence or
   directly audit the earlier range to extend the boundary; advancing the date
   does not close it.
3. **#645 batch failure/retry policy remains an open proposal.** Rust bounded
   channels and optional statement retries do not equal Node's fatal-pool and
   deferred-backpressure contract. Final upstream merge or an independently
   demonstrated Rust defect triggers policy/fault comparison and any port.
4. **#646 keyed/array retirement remains an open compatibility decision.**
   Ordered key locks and retirement-aware return/recovery stay intact. Final
   upstream merge triggers comparison of locked/unlocked array behavior,
   partial failures, reschedule/recovery races, cross-runtime ownership and
   isolated upgrade paths. Applied migration SQL must not be overwritten.
5. **Other proposals retain their individual dispositions/triggers above.**
   #633/#637/#643/#644/#648 cover explicit migration, cache/error propagation,
   keyed scheduling and runner results already handled by Rust's documented
   contracts. #649 requires Rust-specific defaults/recovery guidance. #642,
   #647, #654 and #655 have the concrete design reasons in their rows.
   #640 and #651 remain closed without merge. None is a required merged port.
6. **Braces' former advisory deferral stays closed by withdrawal.** Fresh
   official advisory metadata still gives withdrawal at 2026-10-23T12:53:02Z
   for GHSA-vfj7-8cjw-p6xm. This is not a patched-version claim. The independent
   source-map-js 1.2.2 fix and its oversized-offset/valid-map regression remain.
   Locked UI installation, current npm audit and regression results are recorded below.
7. **Documented runtime differences remain intentional:** opt-in SQL retries;
   no replay of caller-owned transactions or ambiguous execution I/O; missing
   queue names represented by Option; observer containment excludes aborting
   panics; optional local-calendar cron; Rust-specific LocalQueue jitter and
   recovery. Node return SQL does not consult Rust retirement markers.
   Every proxy/OS/PG12 combination, equal migration numbers, different advisory
   lock namespaces and mixed-version recovery are not certified here.
8. **ADR 0004's separate requirements/capabilities model remains a product
   proposal.** Upstream has no merged taxonomy/protocol. Rust preserves accepted
   any-match, forbidden veto and optional every-key allowlist, including operational
   flags. Unblock a separate model with an explicit taxonomy, public protocol and
   rollout contract, or final merged upstream design; do not infer it from #653.
9. **Retirement before revision 22 cannot be reconstructed from existing rows.**
   Exhausted attempts/missing keys do not distinguish retired jobs from legitimate
   final attempts. ADR 0002 deliberately leaves old rows unmarked. Retroactive
   protection needs reliable historical event evidence or an explicit administrative
   decision for known jobs. No production operation or guessed migration is performed.
10. **Former #625 workflow authorization deferral remains resolved.** The landed
    Test workflow covers PG14–18 with fail-fast=false and max-parallel=2. Both
    driver/TLS coverage variants and TLS1.2/TLS1.3 gates remain; no current
    workflow-scope repair is required for this report-only PR.

## Validation and delivery

Only this report changes. No public API, runtime behavior, schema, SQL or test
assertion is modified; no new mirror tests or empty implementation commit is
manufactured. Required `just lint` and `just test-docker` are run before committing.
The default regression suite includes named-backlog success/retry/permanent
failure, mixed unnamed/named backlog, shutdown, key/identity locking, retirement,
cleanup/cache races, cron boundaries/recovery and both revision-20 upgrade fixtures.

There is no uncertain new merged behavior requiring a fresh differential or
performance port. November 5's pinned v0.18.0 versus b4f1b41 named/mixed-backlog
Node differential and 1,200-job Rust drain measurements remain **historical
results**, not measurements from this run. This run verifies source retention
and reruns existing Rust regressions. Hosted exact-head gates retain PG14–18,
both drivers, four coverage configurations, TLS1.2/TLS1.3, OpenTelemetry
features and documentation. No fresh full PG12 or cross-OS certification is claimed.

Completed local results appear below. PR bodies remain empty under the
selected maintenance-delivery skill. Actual review submissions and unresolved
threads are evaluated with exact-head CI before guarded squash merge; main CI
is checked at the actual merge SHA. Final SHA, workflow IDs, decisions and URLs
are preserved in `/home/node/.local/state/graphile-worker-rs-upstream/` and durable
run output. This report is substantive checkpoint documentation, not an empty PR.

### Completed tooling checks

- `just lint` passed with pinned Rust 1.99.1 and three build jobs: formatting,
  all-target checking and Clippy with warnings denied.
- mdBook 0.4.52 built the current documentation successfully.
- All 14 release-tool Python tests and six TLS dependency-isolation scenarios passed.
- Locked UI development dependencies installed with scripts disabled using
  CI-pinned Node 24.22.0. The oversized-offset/valid-source-map regression passed;
  fresh npm audit reported zero vulnerabilities.
- Mechanical coverage verification found all 149 individual first-parent ledger
  rows, all six checked downstream ancestors, unchanged wakeup implementation/tests
  since #581, and no migration SQL diff. SHA256 hashes of all packaged migration
  files and live reference/PR/release snapshots are preserved in run output.

Required database validation uses PostgreSQL 18.6, image
`postgres@sha256:e3d24f6906d0760bf4da0dae872415391ededefad23fd471c47dc53b0ddf3c42`.
The unchanged Docker recipe uses normal durability and UUID-isolated databases;
three build jobs and one test thread limit local resource/timing contention.


### Required complete database suite

`just test-docker` completed successfully: **536 passed, zero failed,
7 ignored**, across 90 test/doc-test suites. All 60 root unit tests,
all eight active named-queue wakeup regressions, migration install/dual-revision-20
upgrade tests, SQL retry/error, cron/backfill, keyed identity/retirement/recovery,
run-once and cleanup/cache contention assertions passed. The seven ignores
include the explicitly invoked manual drain benchmark; no ignored test is
reported as passed. The recipe removed its disposable container successfully.

This is the final local result on unchanged Rust source. Alternate driver/TLS
and PostgreSQL version coverage remains an exact-head hosted CI gate. No new
Node differential, manual benchmark or full local async-std matrix was run for
this report-only change; historical measurements are identified separately above.
