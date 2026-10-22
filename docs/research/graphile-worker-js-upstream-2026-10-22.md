# Graphile Worker upstream audit — 2026-10-22

## Result and separate inspection checkpoint

This audit implements missing SQL retry, queue-name lookup and explicit cleanup
keep-list capabilities found by broadening the historical comparison. It also
fixes an independently reproduced run-once fetch-error defect and an admin UI
build-parser security advisory. There is one
newly merged upstream dependency delta and no newly merged queue implementation.
This report does not claim full parity.

- Downstream baseline: `a422850aecea6bce1c93a7b50d42ca71201e202a` (`leo91000/graphile_worker_rs/main`).
- Previous verified inspection: `4cda192c5df254392a1dff350e5d73f7d2c18a85`.
- Live official main / new inspected checkpoint: `7a2405bdd3b6322fb6b0d48dc0543bcc89166930`.
- Published stable remains **v0.18.0**, `4cda192c5df254392a1dff350e5d73f7d2c18a85`, published 2026-09-08.
- Maintenance `0.17.x` / peeled v0.17.3 remains `ca2cef1c8c6bf9d9c818d19f194f596971c212df`.
- Incremental range: `4cda192c5df254392a1dff350e5d73f7d2c18a85..7a2405bdd3b6322fb6b0d48dc0543bcc89166930`: **one first-parent merge**, two reachable commits including its branch commit. The complete merge diff changes only yarn.lock.
- Historical coverage broadened to peeled **v0.16.0**, `188c330e76e036bf2671e772c9b86b93beb5b31b` (exclusive). New older range through v0.16.6 has **41 first-parent deltas**, individually listed below.
- Retained v0.16.6..v0.18.0 has **105 first-parent deltas**. Fresh enumeration verifies that every full commit URL occurs in the [September 17 ledger](graphile-worker-js-upstream-2026-09-17.md). Each merge represents its complete merge delta. Together the cumulative ledger covers **147 first-parent deltas** from v0.16.0 through current upstream main.

Fetched live official branches/tags, release metadata, CHANGELOG, merged PRs,
commit diffs and source/tests in a read-only comparison clone. Published stable
and merged-but-unreleased behavior are separated below; release objects alone
are not used as the inventory. No upstream writes were made.

The local checkpoint directory was absent on this node. Reconstructed state
from committed September 17/18 and October 8/15 reports, verified ancestry and
current Rust source/tests, and broadened the earlier incomplete boundary. The
September 5 report is still absent from main and fetched downstream history.
That missing report is not treated as proof of earlier implementation coverage.
Current AGENTS.md, architecture, four existing ADRs, compatibility guidance,
justfile and actual CI were read. An isolated worktree preserves the shared
checkout. No competing open upstream-port PR existed; dependency and release
PRs are outside this delivery and were left untouched.

Earlier downstream #514, #524, #547 and #568 landed in baseline. Ancestry and
actual source confirm cron recovery/parser strictness, named-queue claims,
identity preservation, observer containment, retirement-aware recovery, local
queue return durability, driver diagnostics and run-once persistence-error
propagation. Historical reports' descriptions were checked against those current
implementations. #625's former workflow-authorization deferral is closed: the
current Test workflow covers PostgreSQL 14/15/16/17/18. No open blocking review
on an old PR is taken as evidence about the new task head.

## Newly merged but unreleased ledger

| Upstream commit / PR | Changed behavior, affected components and disposition |
| --- | --- |
| [7a2405b / #634](https://github.com/graphile/worker/commit/7a2405bdd3b6322fb6b0d48dc0543bcc89166930), branch [29b73dc](https://github.com/graphile/worker/commit/29b73dcc5dc817399f7dcb5c564f701639a3228d) | **Inapplicable security dependency delta.** yarn.lock upgrades Joi 17.11.0 to 17.13.7 and its address dependency/ranges. The PR includes prototype-injection and date-validation dependency fixes; these are meaningful security fixes for that dependency, but Rust queue crates do not execute Joi, its schema validators, or the Node website dependency graph. Cargo manifests, typed configuration in Rust and the separate admin UI manifests have no Joi dependency. No src/, sql/ or worker-test delta exists between stable and main. Do not call this a published stable change or port npm dependencies into Cargo. |

## Broadened published-stable ledger: v0.16.0..v0.16.6

All entries in this section are included in the current stable v0.18.0. For
source changes, the complete source/test delta was compared, including cleanup,
worker helpers, retry logic, signal handling, task loading and documentation.
Release/website/packaging deltas are distinguished from queue behavior.

| Upstream commit / PR | Changed behavior, Rust components and disposition |
| --- | --- |
| [5650fbc](https://github.com/graphile/worker/commit/5650fbc4406fa3ce197b2ab582e08fd20974e50c) 0.16.6 | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [6ef086c](https://github.com/graphile/worker/commit/6ef086c15bf17d363ebffd10d821dbbab2362adc) Cleanup now allows you to specify additional identifiers to keep (#465) | **Implemented.** Additional cleanup keep identifiers (#465); utils maintenance and CLI. `cleanup_with_keep` merges explicit and local names without registering extras or changing the task cache; CLI `--keep-task` replaces JS task-directory discovery. New identity/cache regression plus existing cleanup concurrency tests. |
| [c89ab35](https://github.com/graphile/worker/commit/c89ab35dbe06469796bcf08935eb994514a0839d) Handle errors on stdout/stderr (#464) | **Already supported / inapplicable runtime details.** Broken-pipe shutdown (#464); shutdown-signal crate and logging. Unix signal listener includes SIGPIPE; shutdown tests cover draining. Node stdout/stderr error EventEmitters do not exist in Rust. Application-owned tracing writers control their I/O errors; the library cannot attach JS error listeners to them. |
| [c0753d4](https://github.com/graphile/worker/commit/c0753d457f3473c5b3caa01ade2c8efff7d20457) Fix cleanup command (#463) | **Already supported.** Null-queue-safe, locked-queue-safe cleanup (#463); utils `types.rs` uses non-null referenced IDs and NOT EXISTS, retaining locked queues. Existing `worker_utils_cleanup/job_queues.rs`, reference and race regressions validate this stronger current implementation. |
| [2bafad3](https://github.com/graphile/worker/commit/2bafad33403a95f97dcb1da921720b25e81acada) website: sponsor update (#462) | **Inapplicable website/funding.** Upstream sponsorship/funding, commercial pricing/Pro page, CSS nesting/build or site links. These change the upstream website and funding copy, not PostgreSQL jobs or the downstream Rust product/API. Rust docs/CLI do not use this Docusaurus runtime. |
| [bb1eb19](https://github.com/graphile/worker/commit/bb1eb1926c83ec945fccb470826a30fdcb203bc9) sponsorship: update sponsors 🙏 (#459) | **Inapplicable website/funding.** Upstream sponsorship/funding, commercial pricing/Pro page, CSS nesting/build or site links. These change the upstream website and funding copy, not PostgreSQL jobs or the downstream Rust product/API. Rust docs/CLI do not use this Docusaurus runtime. |
| [2ea039e](https://github.com/graphile/worker/commit/2ea039ed3423336fdbb9e3738d7813fb5c42c318) docs: fix typos in FAQ (#454) | **Already supported / inapplicable copy.** FAQ spelling (#454); operation guides already describe LISTEN/polling and pool/proxy behavior. No queue or SQL behavior changes; Rust does not copy the Node sponsorship invitation. |
| [c370caa](https://github.com/graphile/worker/commit/c370caa3c216508c389a46261bb05f9213a0a022) Documents WorkerEvents (#446) | **Already supported / intentional API.** WorkerEvents documentation (#446); typed lifecycle observers/interceptors and `docs/src/guides/hooks.md` provide the Rust API. No EventEmitter property names or graphile.config event registration contract exists in compiled Rust. |
| [6772e74](https://github.com/graphile/worker/commit/6772e7437f2852f3ea912071699b353fb86a62d7) Bump express from 4.18.2 to 4.19.2 (#452) | **Inapplicable dependency/packaging.** JavaScript dependency lock/build classification only (Express, follow-redirects, webpack middleware or website dev dependencies). Rust worker crates do not execute those packages. Checked Cargo manifests and separate admin UI package scope; no queue SQL or Rust runtime behavior is introduced by this delta. |
| [5c8c33f](https://github.com/graphile/worker/commit/5c8c33f2a57f57fd41c28476a3d6d26ec3ddd7e9) 0.16.5 | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [748c2ec](https://github.com/graphile/worker/commit/748c2ec1e4c63ed75a7bff36dae0d85f66c27541) Add release note | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [3fa9f06](https://github.com/graphile/worker/commit/3fa9f068069d131cc42f82427177c0b7155af668) feat: recover on timeout (#451) | **Implemented with compatibility decision.** Timeout recovery (#451); database pool acquisition marks ETIMEDOUT before SQL execution. Optional SqlRetryPolicy retries it with current v0.18 exponential jitter. Ambiguous execution-time I/O is not replayed; deterministic classification/cancellation tests and ADR 0005 explain the boundary. |
| [6f61b2e](https://github.com/graphile/worker/commit/6f61b2e8063a3167611390dac90aabdaaf572391) Bump webpack-dev-middleware from 5.3.3 to 5.3.4 (#450) | **Inapplicable dependency/packaging.** JavaScript dependency lock/build classification only (Express, follow-redirects, webpack middleware or website dev dependencies). Rust worker crates do not execute those packages. Checked Cargo manifests and separate admin UI package scope; no queue SQL or Rust runtime behavior is introduced by this delta. |
| [6ba136b](https://github.com/graphile/worker/commit/6ba136b5a90c92b6494d4d8428df561474f439ba) Bump follow-redirects from 1.15.4 to 1.15.6 (#448) | **Inapplicable dependency/packaging.** JavaScript dependency lock/build classification only (Express, follow-redirects, webpack middleware or website dev dependencies). Rust worker crates do not execute those packages. Checked Cargo manifests and separate admin UI package scope; no queue SQL or Rust runtime behavior is introduced by this delta. |
| [e4e4ddc](https://github.com/graphile/worker/commit/e4e4ddc6696dc41ae5b7b6489a8b9f1082735ce7) Apply fix to cleanup docs (#447) | **Already supported.** Cleanup safety documentation (#447); utils CleanupTask docs, job-management/CLI guides explain task-cache risks and lock retention. This audit adds explicit external keep-list guidance without removing warnings about unpreserved other workers. |
| [93e6e60](https://github.com/graphile/worker/commit/93e6e607f8bb37bdabc4faa44c62001b062480c4) fix: add EHOSTUNREACH to list of recoverable errors (#445) | **Implemented with compatibility decision.** Host-unreachable recovery (#445); database adapters walk typed acquisition error sources for HostUnreachable, retaining diagnostics. Only proven pre-execution failures acquire the retry marker; ordinary code/text cannot manufacture a safe retry. ADR 0005 and unit tests. |
| [dd1860a](https://github.com/graphile/worker/commit/dd1860aaeed30bd4aba8edccbba3c981142a7585) 0.16.4 | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [e33771b](https://github.com/graphile/worker/commit/e33771b02658a081c48c64be97e3a0d2b5deeb3f) Move website deps to devDeps (#444) | **Inapplicable dependency/packaging.** JavaScript dependency lock/build classification only (Express, follow-redirects, webpack middleware or website dev dependencies). Rust worker crates do not execute those packages. Checked Cargo manifests and separate admin UI package scope; no queue SQL or Rust runtime behavior is introduced by this delta. |
| [e0c8081](https://github.com/graphile/worker/commit/e0c8081998e056c8eef8d3ca4a570da99f2ec29f) Update lockfile | **Inapplicable dependency/packaging.** JavaScript dependency lock/build classification only (Express, follow-redirects, webpack middleware or website dev dependencies). Rust worker crates do not execute those packages. Checked Cargo manifests and separate admin UI package scope; no queue SQL or Rust runtime behavior is introduced by this delta. |
| [a8cff3f](https://github.com/graphile/worker/commit/a8cff3fa72df4eb3609f3ef488f8a4975a3ebb93) Add support for nested CSS to website | **Inapplicable website/funding.** Upstream sponsorship/funding, commercial pricing/Pro page, CSS nesting/build or site links. These change the upstream website and funding copy, not PostgreSQL jobs or the downstream Rust product/API. Rust docs/CLI do not use this Docusaurus runtime. |
| [e778d42](https://github.com/graphile/worker/commit/e778d4254997aff3a5e3c590586eecd529484aa9) Fix links | **Inapplicable website/funding.** Upstream sponsorship/funding, commercial pricing/Pro page, CSS nesting/build or site links. These change the upstream website and funding copy, not PostgreSQL jobs or the downstream Rust product/API. Rust docs/CLI do not use this Docusaurus runtime. |
| [6ad68db](https://github.com/graphile/worker/commit/6ad68dbd6246a522066ff95dcedc297b17e47c9a) 0.16.3 | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [6b54722](https://github.com/graphile/worker/commit/6b54722af7abda1c54fa2f4c44a2101c15ffb0c5) Add release notes | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [a3d1a85](https://github.com/graphile/worker/commit/a3d1a85fcafb774c80b46583e6be75dbd016738e) docs: fix link to blog post (#436) | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [aabcd85](https://github.com/graphile/worker/commit/aabcd85f74319634f4a41a1de0dda03318200697) website: Pro Page (#442) | **Inapplicable website/funding.** Upstream sponsorship/funding, commercial pricing/Pro page, CSS nesting/build or site links. These change the upstream website and funding copy, not PostgreSQL jobs or the downstream Rust product/API. Rust docs/CLI do not use this Docusaurus runtime. |
| [86bfcc8](https://github.com/graphile/worker/commit/86bfcc841a06e052b48d1c2464d70080e4b0e720) feat: add "cannot connect now" to list of retryable errors (#439) | **Implemented with compatibility decision.** Cannot-connect-now recovery (#439); SQLSTATE 57P03 uses 3s initial/120s capped exponential jitter, with an explicit total-attempt bound. Database policy tests cover classification, timing and final errors; pool backend SQLSTATE conversion is preserved. |
| [aaac77a](https://github.com/graphile/worker/commit/aaac77a88ff73706bc0e932cad35629a37d67576) admin: Update FUNDING.yml (#435) | **Inapplicable website/funding.** Upstream sponsorship/funding, commercial pricing/Pro page, CSS nesting/build or site links. These change the upstream website and funding copy, not PostgreSQL jobs or the downstream Rust product/API. Rust docs/CLI do not use this Docusaurus runtime. |
| [a72f716](https://github.com/graphile/worker/commit/a72f7168fccf5f2d73c9190ec87b953c2c13457d) website: Sponsor update (#431) | **Inapplicable website/funding.** Upstream sponsorship/funding, commercial pricing/Pro page, CSS nesting/build or site links. These change the upstream website and funding copy, not PostgreSQL jobs or the downstream Rust product/API. Rust docs/CLI do not use this Docusaurus runtime. |
| [f3ce61c](https://github.com/graphile/worker/commit/f3ce61c03ab3b00e367e10fef249ff4b2e8b4ca0) 0.16.2 | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [53f83a8](https://github.com/graphile/worker/commit/53f83a82ba6a1430064ff252cbc02dcdff8eee06) Next version number | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [4e44699](https://github.com/graphile/worker/commit/4e44699d8f72113f37ba8089776a859b57d76d2b) Give various SQL commands the ability to auto-retry on serialization failure/deadlock (#430) | **Implemented with compatibility decision.** SQL serialization/deadlock retries (#430); opt-in Database/WorkerOptions policy retries pooled statements, preserving the Rust immediate-error default and caller-owned transaction boundaries. Current #474 exponential jitter supersedes the historical square-root algorithm. Real PostgreSQL claim/outcome faults, both drivers and deterministic clock tests cover it. |
| [0ecf414](https://github.com/graphile/worker/commit/0ecf414a481f355bcaacb0165402f13d2c03012b) Add '--disable-proto=delete' to CI (#429) | **Inapplicable.** Node prototype hardening in Jest CI (#429); Rust values do not use JavaScript Object.prototype and the worker is compiled. Existing Rust/driver/TLS CI remains; this Node flag cannot be passed to Cargo. |
| [e855a5a](https://github.com/graphile/worker/commit/e855a5a5d519927054b108e089ed53112c15c281) Add helper to get queue name of running job (#428) | **Implemented / Rust API adaptation.** Queue-name helper (#428); public `get_queue_names` and context lookup preserve queue identity and input order, duplicates/missing IDs, errors and quoted schemas. Immediate queries replace the JS deferred batch/result cache; no cache performance equivalence is claimed. New queue-name and worker retry integration tests. |
| [2c6fb01](https://github.com/graphile/worker/commit/2c6fb01473a137dd164fb2f3244082466df1a4a4) sponsorship: update sponsors 🙏 (#425) | **Inapplicable website/funding.** Upstream sponsorship/funding, commercial pricing/Pro page, CSS nesting/build or site links. These change the upstream website and funding copy, not PostgreSQL jobs or the downstream Rust product/API. Rust docs/CLI do not use this Docusaurus runtime. |
| [0c6d8dd](https://github.com/graphile/worker/commit/0c6d8dd65c6e6f76cde890135ac837e402615ca7) Release notes, fix env in test (#426) | **Already supported / inapplicable test plumbing.** Release notes and run-once fixture environment (#426). Rust uses isolated DATABASE_URL fixtures and `listen_os_shutdown_signals(false)` test configuration; no Node environment flag is consumed. Run-once tests and helper setup are the evidence. |
| [913d4d8](https://github.com/graphile/worker/commit/913d4d85d7fe98a696953ad700b1f25ff82df3de) Fix loading tasks on Windows (import() URL, not file path) (#421) | **Inapplicable.** Windows JS task loading (#421) changes file paths to module URLs. Rust TaskHandler registrations are compiled functions; no dynamic JS import or task-directory loader is executed. Rust Windows signal support is preserved. |
| [4b75a94](https://github.com/graphile/worker/commit/4b75a94f78b9a4ab4799ee520e8f3ece758e744b) Fix logger scope for worker (#424) | **Already supported / Rust API.** Worker logger scope (#424); Rust uses tracing fields/spans and structured worker/job identity instead of nested JS logger scope objects. Current runner logging and error chains retain identity; no JS scope object exists to unwrap. |
| [0d71581](https://github.com/graphile/worker/commit/0d7158195bcf136154c94fab1eaac28d59b0c933) Add "database cleanup" feature (#415) | **Already supported.** Cleanup API/CLI (#415); Rust enum tasks, public cleanup, CLI maintenance and dedicated cleanup integration suites cover unused queues/tasks and unlocked permanent failures. Current lock/snapshot/cache safeguards remain. JS pool releaser test plumbing maps to owned Rust handles and panic-safe fixture cleanup. |
| [0257b2b](https://github.com/graphile/worker/commit/0257b2b21649d024ca74534e6e3dc293d8350166) Bump follow-redirects from 1.15.3 to 1.15.4 (#422) | **Inapplicable dependency/packaging.** JavaScript dependency lock/build classification only (Express, follow-redirects, webpack middleware or website dev dependencies). Rust worker crates do not execute those packages. Checked Cargo manifests and separate admin UI package scope; no queue SQL or Rust runtime behavior is introduced by this delta. |
| [8f503c2](https://github.com/graphile/worker/commit/8f503c25c7a96526a9295579dfe7133ea55f208c) 0.16.1 | **Inapplicable release metadata.** npm version/source-version or release-note/link metadata; no independent SQL/queue behavior. Behavioral source changes in those releases have individual rows in this ledger. Rust crate versions and release automation are retained; no manual release/tag is performed. |
| [383cc41](https://github.com/graphile/worker/commit/383cc41d04a977166d358161b48aa49a6859d3b7) Fix ESM woes (#417) | **Inapplicable.** ESM/CommonJS TypeScript export discovery (#417); Rust compiled TaskHandler registration does not import module exports. The changed JS fixture exports have no Rust runtime equivalent. |

## Retained behavior rechecked against current Rust

The 105-entry historical ledger remains cumulative, with its September 18,
October 8 and October 15 corrections. These comparisons update its applicable
items rather than inferring parity from similar names.

| Upstream source change | Current comparison and disposition |
| --- | --- |
| #622 / e9f3e3f: named-queue batch claims | **Already supported.** `batch_get_jobs.rs` locks candidates before DISTINCT ON queue identity, retaining individual unnamed jobs and binding the candidate limit. `tests/upstream_sync.rs` checks sibling state, priority and concurrent claimers. Flag routing precedes the limit. |
| #623 / be38db6: identity allocation | **Already supported.** Rust migration 21 and `task_identifiers.rs` filter existing names before INSERT; revision 22 retains the SQL. Repeated/exhausted identity and dual revision-20 upgrade tests remain. |
| #627 / 9fc851b: SQL/helper allocations | **Already supported / Rust design.** `fetch_query_cache.rs` bounds SQL text to 128 entries per thread, keyed by schema, all filter shapes, clock and fetch mode. No values, results or connections are cached. owned context handles in Rust replace JS closure rebuilding. No new throughput claim. |
| #557 / 90ea860: observer errors | **Already supported.** `events/observer.rs` catches both construction and future-polling panics; lifecycle tests retain both failure sites and repeated emission. Aborting panics/application side effects remain outside rollback. |
| #474 / 5a305b9, including 0bd0b77: batching, persistent statements and retry calculator | **Supported capabilities plus retry gap implemented here.** Prepared-statement opt-out still reaches SQLx direct/transaction paths. Current upstream retry source changed the earlier square-root policy to exponential jitter; `SqlRetryPolicy` implements the current constants/calculator with opt-in Rust semantics, safe acquisition classification and caller-owned transaction exclusion. This corrects incomplete historical coverage of that mixed diff. Rust's bounded channels and per-job fallback for failure batches remain, and asynchronous batch error policy remains different. |
| #612 / 7de3c85 and #613 / 58d1ed2: failure IDs | **Already supported.** Batch failure parameters use I64Array/bigint[]; upstream-sync tests check IDs above i32::MAX with/without named queues and release state. |
| #544 / b94bcf8: cron recovery | **Already supported.** CronRunner retries registration/backfill from 200ms to a 60s cap, resets on a healthy tick and races the whole loop against shutdown. Clock-driven recovery, timezone/boundary and backfill tests remain. Its outer scheduler retry policy is deliberately separate from optional SQL statement retries. |
| #543 / bb03d1c: nested cron identifiers | **Already supported.** Parser tests preserve options, JSON payload and subsequent entries; whole-input/location validation remains. Compiled task registration is intentional. |
| #502 / 0ec5c7e: concurrent schema initialization | **Already supported.** Migrator retries recognized initialization clashes and propagates unrelated errors. Concurrent/schema-install tests remain; transaction replay is not inserted into applied migrations. |
| #625 / 9dcec76: PostgreSQL versions | **Already supported.** PG14–18, max-parallel=2 and fail-fast=false are present. Node/Jest versions and OS module-loader matrices do not transfer. PG12 remains documented minimum; no fresh full PG12 suite is claimed. |
| #620 / cea9d60, #569 / 4207e7d, #558 / 91c950f: attempts, arrays and queue cardinality | **Already supported.** Job types document consumed attempts; keyed SQL concatenates arrays and preserves run_at only on first attempts. Scheduling/queue guides and keyed/batch tests retain those semantics and discourage random queue names. |
| #478 / f26dafe and 1a07517: restricted roles | **Already supported.** Operations migration guide separates migrator/owner privileges from restricted worker/service roles. No production privileges or queue data were changed. |
| JS-only ledger items (each individually pinned in September 17 report) | **Inapplicable runtime/build contracts retained.** Compiled Rust has no ESM/CommonJS task loader, TS type stripping, cosmiconfig/graphile.config discovery, JS Promise/AbortSignal helpers, executable-file task stdout launcher, EventEmitter, Symbol.asyncDispose, npm/Joi/webpack/yarn publishing or upstream sponsorship site. Rust tracing/types, explicit awaited shutdown and its own CLI/builder APIs remain. Operational/SQL content in mixed diffs is accounted for separately above. |

Rust migrations 1–22 are byte-for-byte unchanged. Retirement markers still keep
replaced/removed/permanently failed jobs exhausted during attempt-restoring
recovery; explicit rescheduling clears them after row locking. LocalQueue still
retains uncertain returns and excludes same-worker pending claims. Existing
retirement/concurrency, return cancellation, heartbeat/slot-drain and migration
upgrade regressions are retained. No SQL history is rewritten or version minimum
raised by this audit.

## Open/closed proposal tracking (not merged behavior)

The proposal source/test patches were inspected at these heads. New proposals
can reveal independent Rust defects, but do not establish merged requirements.

| Proposal / inspected head | Rust comparison and disposition |
| --- | --- |
| #633 `13679fe51b7751597db8dc1ffebaf8a69f58e2cb` | **Open; track.** Explicit migration entrypoints/error cleanup. WorkerUtils construction does not migrate; its migrate method is explicit; WorkerOptions init migrates before registration. owned Rust handles and Results replace Node releaser arrays. |
| #637 `3925e59a44d9f0d2d3fda2445053183d4c093ed3` | **Open; track.** Rejected/stale task-cache completion. Rust queries return fresh Results, init shares success only, and cleanup refresh compares snapshots under the write lock and bounds contention rather than overwriting concurrent registration. Existing race tests verify this. |
| #640 `74ccbfd8c8780e5ca9b81ed3ee5924f8c87e0375` | **Closed unmerged on October 18.** Alternate cache rejection fix; retain #637 tracking, not a merged port. |
| #642 `6c6b57897d303db3713e9b9bf3c21d4e3666f725` | **Open; inapplicable.** Upstream sponsorship copy does not establish Rust sponsorship or worker behavior. |
| #643 `11201b5e52a64f533b62a99c1d01bfdab8e608cc` | **Open; track, existing Rust fixes confirmed.** Direct outcomes await and propagate errors; last week's #568 preserved run-once persistence errors and closed-channel failure fallback errors. This audit separately fixes silently swallowed claim errors. Asynchronous batch persistence failures still follow their existing logging/fallback policy. |
| #644 `7db0b5ff0cd36d2283b65e0eaee20bbad695a9be` | **Open; track.** Ordered advisory-key serialization. Rust migration 19 introduced sorted advisory locks and row locks, retained through 22, plus GWNOR missing-result guard. Existing contention regression for bulk job keys validates results and one surviving key. Different advisory namespaces do not prove mixed Node/Rust producer coordination. |
| #645 `0a77c0704669a0a06adda3c5ff504c8507889e58` | **Open; track with differences.** Node batch retry-loop/backpressure-deferred repair. Rust uses bounded channels; it has no matching break/deferred-refcount protocol. Optional pooled statement retries now provide a distinct capability, but do not make Rust's asynchronous batch failure/shutdown policy equivalent to the proposal. Re-evaluate final merged behavior and fault cases if it lands. |
| #646 `5105a7a40e9ace7ed0d6bd75323459c9d1721309` | **Open; track with compatibility decision.** Keyed add/claim race and retry attempts for locked array payloads. Rust already takes keyed advisory and row locks before retiring old rows; existing keyed contention tests remain. The proposed array retirement exemption is absent from stable upstream and differs from Rust revision-22 retirement semantics. It is not silently imported into an applied migration. Re-evaluate final merged SQL, partial batch failures, recovery and upgrade paths when merged (ADR 0005). |
| #647 `1bf3da11c43711975f9cfd4f75a53f2b43df0c32` | **Open; mixed tooling disposition.** yarn.lock updates seven npm packages (follow-redirects, handlebars, http-cache-semantics, Joi, shell-quote, source-map-js and undici). These are not merged queue changes. Rust workers do not execute the Node dependency graph, but the downstream admin UI Tailwind build independently uses vulnerable source-map-js 1.2.1. This audit patches that shared development dependency to 1.2.2 based on the public advisory, not on the proposal's open status. Other listed npm changes have no affected Rust queue/runtime component in this delta. |
| #648 `91ef5723e730b48de6b563b04cd2d1800e6c64b2` | **Open; track; independent Rust fetch fix implemented.** Node runner.promise/CLI fatal-error propagation. Rust continuous run already returns source errors after draining, requests shutdown and keeps the original error if deregistration also fails. Comparison exposed run_once's discarded stream/follow-up fetch errors; three new live PostgreSQL regressions reproduce and fix that defect. No JS runner promise API is added. |

## Implemented behavior and compatibility

See [ADR 0005](../adr/0005-sql-retries-and-upstream-016-audit.md) for the retry,
queue-name, cleanup and error-propagation decisions. Optional retries preserve
original SQLSTATE/diagnostics and bindings, stop on cancellation/exhaustion, use
current upstream exponential jitter, and exclude caller-managed transactions,
commits, raw driver access and LISTEN. The immediate-error default is preserved;
this is an explicit Rust difference, not a claim of default policy parity.

The run-once worker uses an internal fallible claim stream which emits an error
once and terminates. It retains the first fetch or persistence error and drains
active jobs; its named-queue follow-up now also retains errors. Public Job-only
streams and existing public error variants/signatures remain. A rollback of a
failed claim consumes no task attempt or ownership. All existing outcome-failure
assertions remain.

Queue lookup uses a bound int[] and escaped schema, preserves input order,
duplicates and missing IDs, and does not retain a result cache. Cleanup explicit
keeps combine with local names but do not register extra handlers or pollute the
worker task cache. No benchmark claim is made: these are correctness/API ports,
not a new performance optimization. Upstream attribution/license notices are
preserved, including the database crate's retry-policy attribution.

## Validation and delivery evidence

Baseline Check/Test/Coverage succeeded on `a422850aecea6bce1c93a7b50d42ca71201e202a`:
[Check](https://github.com/leo91000/graphile_worker_rs/actions/runs/39524652748),
[Test](https://github.com/leo91000/graphile_worker_rs/actions/runs/39524652686),
[Coverage](https://github.com/leo91000/graphile_worker_rs/actions/runs/39524652730).
These establish baseline health, not the new PR's checks.

The three new fetch regressions all fail against the exact baseline run-once
and stream files because run_once returns Ok. After the fix, initial fetch,
stream fetch with a gated sibling, and named-queue follow-up all return the
original SQLSTATE 40001/diagnostic after healthy work persists. UUID fixtures
verify rollback, zero consumed attempts, and unlocked queue/job state.

The source-pinned differential harness extracts actual v0.18.0 policy,
task-registration, fetch, completion/failure and queue-name functions, uses
upstream's locked pg 8.11.3, and installs all 20 official migrations on four
isolated PostgreSQL databases. Only sleep/randomness are controlled (sleep
records delays, random is pinned to midpoint). With the same trigger fixtures
as Rust, named/unnamed completion/failure each require three claim attempts and
three outcome attempts, one consumed task attempt, expected retained/deleted
jobs, and no queue locks. Node records [50,75,50,75]ms delays at midpoint. Rust
integration tests verify the corresponding rollback/persistence states and
single outcome hooks; deterministic policy tests pin midpoint/extreme jitter,
caps, classification, cancellation and transaction boundaries. This compares
selected source functions and fixtures, not complete cross-language runners or
all mixed-version deployments. No performance comparison is claimed.

Local checks use pinned Rust 1.99.1, two build/test threads and disposable
PostgreSQL. Exact final command results, PR/head/review/workflow IDs and
post-merge evidence are recorded in the durable task checkpoint/run output and
updated below before delivery. All applicable PR checks, actual review decisions
and exact merge-SHA main CI remain required gates. Release automation is left
intact; no manual release/tag or production queue operation is in scope.

## Unresolved items retained separately from the checkpoint

- **Missing pre-v0.16.0 coverage.** The September 5 report and its unknown gap
  list could not be recovered from fetched history. This audit remedies the
  v0.16.0..v0.16.6 boundary by inspecting 41 older deltas and implementing three
  missing capabilities. Earlier behavior is still uncertified. Recover the
  missing report or explicitly extend the comparison before that lower bound;
  advancing the inspected head must retain this limit.
- **Open proposals are watch items, not deferred merged ports.** #633, #637,
  #643–#646 and #648 require a fresh final-diff/compatibility assessment if they
  merge. #642/#647 remain upstream website/dependency scope; #640 is closed.
  #634 is now merged and separately disposed above.
- **Intentional differences remain.** Retry enablement/transaction ownership,
  asynchronous batch failure policy, immediate queue lookups, compiled task
  loading, typed hooks, cron local-calendar behavior, explicit shutdown and
  retirement-aware recovery are documented. Do not certify them as identical
  Node APIs/defaults. No matching applicable merged change is deferred solely
  for size or refactoring cost.
- **Validation limits.** The full local suite does not establish all OS/proxy,
  PG12, full cross-language runner or arbitrary mixed-version combinations.
  PG14–18 and TLS/coverage variants are checked by actual current CI. Main CI
  must be verified after merge, not inferred from PR checks or local success.


### Final local validation before PR creation

- `just lint`: passed formatting, all-target check and Clippy with warnings denied.
- `just test-docker`: **520 passed, 0 failed, 6 ignored**.
- `just test-runtime runtime-tokio driver-tokio-postgres`: **283 passed, 0 failed, 2 ignored**.
- `just test-runtime runtime-async-std driver-sqlx`: **518 passed, 0 failed, 6 ignored**.
- Database all-features unit/driver-contract tests: **20 passed, 0 failed, 0 ignored**.
- All three OpenTelemetry feature checks passed; Python release-tooling regressions and all six TLS feature-isolation checks passed.
- Standalone database crate without default features checked successfully; mdBook 0.4.52 built the guides.
- Real async-std retry execution without a Tokio runtime passed. The full async-std matrix also exercises the supported feature combination; its Tokio-authored integration fixtures are not presented as independent async-std process runners.
- Applied migration SQL 1–22 is unchanged. Existing isolated upgrade, retirement/concurrency, cleanup, retry/backoff, cron, shutdown and cancellation regressions passed with the rest of these suites.

Validation uses Rust 1.99.1 and PostgreSQL 18.6, image digest
`sha256:edb0c08dc33befd0262ab3b5cf1c4f3f8cfd8b09e88c05b45adee29d6761ddb6`.
Two build/test threads were used; the standalone tokio-postgres matrix used one
test thread as its required recipe specifies. After the initial 518-test full
pass, final validation added explicit transaction-boundary and disabled-policy
checks. Generated build output was cleaned through Cargo and debug information
was disabled to fit all matrices on this node; assertions, test selection and
supported-platform coverage were not weakened. One in-progress compile attempt
failed before API wiring was complete; it is not a behavioral reproduction or
a passing gate. The clean final validation run above passed.

The differential fault SQL is shared in `tests/fixtures/sql_retry_faults.sql`;
its whitespace-trimmed SHA256 is
`ef9903c54cff8cb39e3ff77a3a8b4127779e03cd05d8d5985357ca70438fc9a9`.
The source manifest, extracted pinned functions, harness and results are
preserved with the durable run evidence. PR checks, actual final-head review
and guarded merge/main CI are subsequent delivery gates, recorded in the final
checkpoint and run report rather than inferred from these local passes.


### Admin UI build-parser security follow-up

The branch push surfaced existing downstream Dependabot alert #2:
[GHSA-68fv-2mgg-jv7q / CVE-2026-93749](https://github.com/advisories/GHSA-68fv-2mgg-jv7q).
Unlike the worker's Rust crates, admin UI asset generation does execute
source-map-js through `@tailwindcss/node` 4.3.3. The installed lockfile contained
1.2.1. The Rust HTTP server serves prebuilt CSS/WASM and does not parse these
source maps; this is a build/development dependency fix, not a queue SQL change.

The lockfile now resolves 1.2.2, the first patched release. Its official source
fix [cf765805](https://github.com/7rulnik/source-map-js/commit/cf7658058ceeaa8619d5ae0ec90be6905209d016)
validates indexed section offsets and bounds large/nested offsets, avoids
repeated nested source enumeration and improves large line-gap handling. These
behaviors belong to that dependency; no copy is ported into Rust. All top-level
Tailwind/icon pins remain unchanged, and no shipped/prebuilt UI asset changes.
This independent advisory finding also corrects the overly broad initial
inapplicable disposition of open upstream #647's shared source-map dependency.

`node .github/scripts/check_source_map_offsets.cjs` after the admin UI's
`npm ci --include=dev --ignore-scripts --no-audit --no-fund` rejects an indexed
map with offset line 10,000,001 and checks an ordinary indexed map resolves its
source/line. The old 1.2.1 constructor accepts the oversized offset, reproducing
the failure safely without expanding a malicious map. An initial normal-case
probe used an unmapped indexed-map column; that fixture was corrected to column
1. Baseline failure is specifically missing offset rejection, not that fixture
mistake. Tailwind builds before/after the lock update produce byte-identical
CSS. Fresh required Rust validation and final-head PR review/checks apply to
this follow-up as well. No manual release, package publication or deployment is
performed.


### Review of the initial delivery head

[PR #576](https://github.com/leo91000/graphile_worker_rs/pull/576) initially used
head `5fa159f84abd3d71100e6f26c4e4bcc950a515f6`. Its actual review was
**COMMENTED**, not APPROVED or CHANGES_REQUESTED. It reported no actionable code
findings, two unverified wording suggestions, and insufficient docstring
coverage (65.22% against 80%). The wording was clarified and affected database
construction/conversion, pooled execution, retry defaults, queue lookup and
mock-driver methods now document their contracts. Function bodies are unchanged
by that documentation update. The suggested PR description remains inapplicable:
the selected delivery skill explicitly requires an empty body; detailed evidence
is committed here. No successful bot status is treated as approval.

Initial Codecov checks succeeded with 98.23789% patch coverage and 97.4% project
coverage. Four uncovered changed lines were reported; thresholds, exclusions
and assertions remain unchanged. The security follow-up and documentation need
fresh-head checks and review. The review service reports one included review
per hour, with no allowance remaining immediately after this review; a quota
reset must be honored before explicitly requesting any stalled re-review.

### Remaining development dependency advisory

After patching source-map-js, `npm audit` reports four high entries propagating
one unpatched [braces advisory, GHSA-vfj7-8cjw-p6xm](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm)
through micromatch, @parcel/watcher and @tailwindcss/cli. The live registry's
latest braces is 3.0.3 and the advisory lists no patched version. npm proposes a
Tailwind downgrade from pinned 4.3.3 to 4.3.0; no forced fix or pin change is
made. The affected watcher/glob tooling operates on repository-controlled build
patterns; the Rust worker/server does not parse job payloads as brace patterns,
and this task does not enable watch mode.

**Disposition: deferred development-tooling update, with an exact external
unblock condition.** Revisit when a compatible patched braces version is
published, or an approved compatible Tailwind/watcher dependency update removes
that code. This is independent of the applicable merged queue ports and does
not erase the alert or claim an audit with zero npm advisories. Source-map-js
is separately fixed with its reproduced regression and unchanged CSS output.


### Follow-up local validation

After the source-map lock update and review documentation changes, `just lint`
passed and `just test-docker` again completed with **520 passed, 0 failed,
6 ignored**. Recovery inspection verified the completed logs and container
cleanup before any rerun. The bounded parser regression passed with 1.2.2; its
1.2.1 baseline failed specifically at the oversized-offset rejection. mdBook
built successfully. Both CSS builds were 42,530 bytes with SHA256
`674236a02f506c6a1cc8a44d2c591091b7baf55c29d7ac423104d17df03c293a`.


### Full re-review follow-up

The explicitly requested review of head `1024f6133e3ebae525de0b76a99d47fc0fd0aff9`
returned **CHANGES_REQUESTED**, despite successful bot/check statuses. All three
findings were evaluated against current source. The source-map regression is
now run by the Check workflow after installing the locked UI development
dependencies (scripts disabled), and ADR 0005 explicitly distinguishes job
retry-attempt accounting from SQL statement retries.

The valid run-once finding exposed continued named-queue claims after a sibling
fetch error. A causal gate around the real PostgreSQL driver reproduces it on
the prior head: the pending healthy job was consumed instead of remaining
available. Run-once now shares a fetch-stop signal between stream and named
continuations, records stream errors before polling another ready continuation,
and drains handlers already running. Claims already in flight are awaited and
returned through the existing retirement-aware return SQL before another
handler is started. Their consumed attempts and queue locks are restored;
claim cancellation is avoided because an autocommit may already have committed.
Ordinary processing/persistence errors do not set the fetch-stop signal.

Three regression scenarios cover the pending named backlog, a committed
continuation claim completing after a stream error, and a committed stream
claim completing after a continuation error. Explicit notification gates and
five-second outer bounds replace timing assumptions. The first stop-signal
implementation additionally failed the in-flight continuation scenario because
it signalled only in the queued error handler; moving publication to the stream
observation point fixed that ordering defect. All 25 run-once tests then passed.
The real drivers and complete required suites are validated again for this
follow-up; delivery evidence records the final-head checks and actual review.


Required validation after these review fixes: `just lint` passed and
`just test-docker` completed with **523 passed, 0 failed, 6 ignored**. mdBook
built successfully, and the exact new CI npm-install/parser-check commands
passed locally. The added workflow uses the repository's existing Node.js
24.22.0 release-workflow version and an officially verified setup-node v7 SHA;
local npm/parser execution used the installed Node.js 24.21.0. No project pin
was upgraded by this follow-up.


The full follow-up `runtime-tokio`/`driver-tokio-postgres` matrix passed:
**286 passed, 0 failed, 2 ignored**. The full `runtime-async-std`/`driver-sqlx`
matrix passed: **521 passed, 0 failed, 6 ignored**, with the same fixture/runtime
limitations explained above. Both include all six fetch-error scenarios. The
isolated PostgreSQL fixture was removed after these runs; no queue schema or
applied migration changed in the review fixes.
