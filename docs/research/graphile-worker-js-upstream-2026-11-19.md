# Graphile Worker upstream audit — 2026-11-19

## Inspection checkpoint

Live upstream main is unchanged since November 12. Published stable remains
**v0.18.0**. This week's Rust ports come from a broadened historical review
of published releases. No merged-but-unreleased behavior required a new port.

| Reference | Verified value |
| --- | --- |
| Downstream main at investigation start | `757ced2731b95e78db81d998503f72c6fff83203` |
| Previous and current upstream main | `b4f1b41c7abd747586879bf68a46a5a22897fa96` |
| Weekly inspected range | `b4f1b41c7abd747586879bf68a46a5a22897fa96..b4f1b41c7abd747586879bf68a46a5a22897fa96`, zero commits |
| Published v0.18.0 | `4cda192c5df254392a1dff350e5d73f7d2c18a85`, 2026-09-08 |
| Merged but unreleased | `4cda192c5df254392a1dff350e5d73f7d2c18a85..b4f1b41c7abd747586879bf68a46a5a22897fa96`, 3 first-parent deltas, 11 reachable commits |
| Historical audit extension | `a6a52b4948e1f85d71211ff516b4f061a71f2167..188c330e76e036bf2671e772c9b86b93beb5b31b`, v0.13.0 exclusive through v0.16.0 inclusive, 104 first-parent deltas |
| Cumulative first-parent coverage | v0.13.0 exclusive through current main, 253 deltas |
| Published v0.14.0 tag off the main line | `a79531444f1c80bc962f0dc0fd7efc8528bd85f8`, one tag-only commit |
| Published v0.15.0 and v0.15.1 | `aae7527fe4a2e3464bb2f198a228c4219ffc6cd9`, `a2f8e0a09ca228e092b5ac5a9367f7db699e8634` |
| Official maintenance refs | `0.13.x` `bf3d2f5214c31627b34f5c5a40a8d881ea81ce8e`, `0.15.x` `3c6c5a56f3f17074d3d21a4f9f00cd513f63c156`, `0.17.x` `ca2cef1c8c6bf9d9c818d19f194f596971c212df`. All are ancestors of upstream main. |

Fetched official Git history, branches, tags, merged PR metadata, open PR
heads, GitHub release notes and npm metadata. npm latest is 0.18.0 and its
gitHead equals the official tag. Main and v0.18.0 are unchanged and contiguous
with the November 12 checkpoint. Read the Rust architecture, ADRs 0001–0006,
previous comparison reports and current CI and testing instructions.
There was no overlapping open upstream-sync PR. Baseline Check, Test, Coverage,
Docs and Release-plz succeeded at the exact Rust SHA, runs 43191937206,
43191937365, 43191937434, 43191937304 and 43191937172.

The September 5 file and local state were absent. Fetched Rust history also
contains no September 5 report. The initial Rust commit `0cd3b97` includes
an older SQL lineage through migration 13. The historical comparison starts
at v0.13.0 (which shipped through migration 10), so it includes the upstream
migration 11–13 changes underlying that initial import as well as subsequent
releases. The historical ledger uses complete merge deltas relative to their
main-branch parent, accounting for changes that survived the merge. It counts
neither intermediate PR snapshots nor their reverted changes as separate
delivered behaviors. The separate v0.14.0 tag commit only changes package
version metadata. The same version bump also appears on the main line in
`4138f75`. Generated SQL packaging landed on main after the tag.

## Previously inspected changes

The [October 22 published ledger](graphile-worker-js-upstream-2026-10-22.md#broadened-published-stable-ledger-v0160v0166)
and [September 17 published ledger](graphile-worker-js-upstream-2026-09-17.md#change-ledger)
account for the next 146 first-parent deltas. The
[November 12 ledger](graphile-worker-js-upstream-2026-11-12.md#merged-but-unreleased-ledger)
accounts for all 11 reachable commits after v0.18.0, including the three
first-parent deltas. The full stable-to-main diff and the dispositions were
rechecked this week:

| Upstream PR | Changed behavior and current Rust disposition |
| --- | --- |
| [#634](https://github.com/graphile/worker/pull/634) | Joi/address lockfile change. Inapplicable to typed Rust options and the independent admin UI. |
| [#652](https://github.com/graphile/worker/pull/652) | Corrects apt setup and pg_dump for Node CI. Rust uses service containers and pinned SQL fixtures. PG14–18 and two TLS protocol tests remain CI gates. |
| [#653](https://github.com/graphile/worker/pull/653) | Named-queue progress after committed outcomes. Rust #581 is in main at `9fb97f3e356b810d86491cecc4a18264efd61eab`. The same direct/batched paths record only persisted release hints, LocalQueue consumes counted hints, and shutdown precedes new claims. Existing mixed, named-only and persistence-fault tests still cover these paths. |

Earlier downstream ports #514, #524, #547, #568, #576, #581 and #582 are
ancestors of `757ced2`. Current source still contains cron retry/backfill,
revision-20 install and upgrade coverage, named-queue batch locks, ordered
key locks and retirement recovery, SQL shape caching and identity preservation,
opt-in SQL retry, task cleanup keep names, queue lookup, prepared-statement
opt-out, run-once error propagation, original PostgreSQL diagnostic sources and
named-queue wakeups. The [November 12 source ledger](graphile-worker-js-upstream-2026-11-12.md#published-stable-behavior-rechecked-on-current-rust-main)
lists query and test entrypoints for those ports. This week's port changes the
cron selection/observation path and the migration control path, with migration
revisions 1–22 retained byte-for-byte.

## Historical published-stable ledger

The interval includes the main-branch changes published by v0.16.0. Release
metadata, bridge-tag metadata and packaging are identified separately in the
dispositions. The preceding v0.13.0 cron release also introduced the
[text duplicate-ID validation and weekday rules](https://github.com/graphile/worker/pull/163). Upstream corrected
weekday matching in [#216](https://github.com/graphile/worker/pull/216).
The Rust text parser now rejects duplicate default or explicit IDs and accepts
Sunday 0 and 7. Rust keeps its existing intersection of weekday and month-day
filters and its typed weekday step offsets. ADR 0007 records the choice.

| Upstream delta | Behavior, affected component and disposition |
| --- | --- |
| [9a838a3073f6c8757131c0f3732ffe42e04ba68e](https://github.com/graphile/worker/commit/9a838a3073f6c8757131c0f3732ffe42e04ba68e) chore: fix anchor in README | README anchor correction. **Inapplicable.** Rust README and mdBook have separate anchors. |
| [d4e2e41daf800def409b4fc770e5dc445e28982f](https://github.com/graphile/worker/commit/d4e2e41daf800def409b4fc770e5dc445e28982f) sponsorship: update sponsors 🙏 (#261) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [4064051a3945a814eac26db0df2b4663fd740bf2](https://github.com/graphile/worker/commit/4064051a3945a814eac26db0df2b4663fd740bf2) docs(readme): Clarify `maxAttempts` (#267) | Explain maxAttempts as total executions. **Supported.** JobSpec and scheduling guide describe total attempts and retry exhaustion. |
| [32079aaa7b1abe1099a9aef0bb71834e4fffd8d2](https://github.com/graphile/worker/commit/32079aaa7b1abe1099a9aef0bb71834e4fffd8d2) sponsorship: update sponsors 🙏 (#268) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [79f21604735af66ac371728c378b1c56b5dcfcc7](https://github.com/graphile/worker/commit/79f21604735af66ac371728c378b1c56b5dcfcc7) fix(cron): avoid unhandled rejection (#263) | Avoid a rejected startup promise even when cron exits immediately. **Supported.** CronRunner races an owned future against shutdown and retries SQL errors. Cron recovery and shutdown tests cover both. |
| [abcb9deef955e42d51d9ff55ffe2716a07973c7f](https://github.com/graphile/worker/commit/abcb9deef955e42d51d9ff55ffe2716a07973c7f) chore: internal refactor of cron (#271) | Split cron matching, registration and runner internals. **Supported.** Rust separates types, parser, backfill and scheduler SQL. JavaScript interface reshaping is inapplicable. |
| [c1b384be13bcae5e9c5728cc43ead686064d6b56](https://github.com/graphile/worker/commit/c1b384be13bcae5e9c5728cc43ead686064d6b56) feat(cron): allow arbitrary functions in place of the cron pattern (#272) | Accept a predicate in programmatic cron entries. **Implemented.** WorkerOptions and CronRunner use the same predicate for historical and live ticks. |
| [14d4fc947ee0555f5efa387ea077568482ae60d5](https://github.com/graphile/worker/commit/14d4fc947ee0555f5efa387ea077568482ae60d5) chore: remove experimental flag from cron (#206) | Make cron non-experimental and remove opt-in environment variable. **Supported.** Cron is configured by WorkerOptions without a feature switch. |
| [543bb46a4cc3aa287a3bff089ad8cdefab25a24a](https://github.com/graphile/worker/commit/543bb46a4cc3aa287a3bff089ad8cdefab25a24a) chore(deps): upgrade deps; reset yarn.lock; Node 14+ (BREAKING) (#270) | Update dependencies and require Node 14. **Inapplicable.** Neither the worker nor the independent Rust UI depend on this runtime or lockfile. |
| [63f4923f20e3faa3f228f149b10dcebad1212a7f](https://github.com/graphile/worker/commit/63f4923f20e3faa3f228f149b10dcebad1212a7f) chore(tests): make more resilient to deferreds (#275) | Use more resilient deferred assertions in Jest. **Inapplicable harness.** Rust uses owned futures, bounded waits and observed mock clocks in tests/cron. |
| [340638ca817105341b375a58b8f6f9aea938b1e3](https://github.com/graphile/worker/commit/340638ca817105341b375a58b8f6f9aea938b1e3) chore(tests): consistently use getJobs/getJobQueues (#276) | Centralize fixture job and queue access. **Supported.** TestDatabase wraps those queries for UUID-isolated databases. |
| [5e47bd871c227df663a38cf7417aa08cc6a625f4](https://github.com/graphile/worker/commit/5e47bd871c227df663a38cf7417aa08cc6a625f4) chore: update perfTest (#277) | Revise perfTest scripts. **Inapplicable harness.** Rust has Criterion and queue-wakeup drain benchmarks. No throughput equivalence inferred. |
| [fc58b7f0605350c224a7d2c9f765836c8f248691](https://github.com/graphile/worker/commit/fc58b7f0605350c224a7d2c9f765836c8f248691) sponsorship: update sponsors 😻 (#281) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [b64b4b1cae3ed8f7b8e467e6ad77e43e409cb978](https://github.com/graphile/worker/commit/b64b4b1cae3ed8f7b8e467e6ad77e43e409cb978) feat(perf): significantly improve "large jobs table" performance (#274) | Normalize task/queue identities, generated availability and partial indexes, move claims/outcomes into queries, migrate old jobs. **Supported.** SQL 11 and subsequent revisions, get_job/batch_get_jobs and release queries implement these mechanics. Migration 11 and upstream_sync exercise ownership, upgrade and keyed data. No new benchmark claim. |
| [9288e0710255b0172192220716890d0c6e93f67e](https://github.com/graphile/worker/commit/9288e0710255b0172192220716890d0c6e93f67e) feat: batch jobs (#286) | Array payload execution and partial retry, keyed array merging. **Supported with typed API.** BatchTaskHandler and failure replacement preserve failed items, with bounded concurrency and tests/batch_jobs coverage. SQL 22 handles array merging. Explicit Rust batch handler declaration preserves scalar handler behavior. |
| [9d484820b97f0f4f677dccdf810905547ee2e74c](https://github.com/graphile/worker/commit/9d484820b97f0f4f677dccdf810905547ee2e74c) fix: don't log worker options even in debug mode (#287) | Remove default logging of worker options that may contain credentials. **Supported.** Builder does not debug-print its options or URL, and Database Debug omits the driver. Job-payload tracing has a separate documented policy. |
| [054f07c33ae0d76f80b7dbaaa89b11dacfae17e2](https://github.com/graphile/worker/commit/054f07c33ae0d76f80b7dbaaa89b11dacfae17e2) chore: add tests for new batchJobs feature (#288) | Exercise array partial failure and retry payload. **Supported.** Batch failure/retry_payload and partial tests cover persisted subsets and exhausted jobs. |
| [af85e51e9da62ebb811bc867eb2d5a827c01cd62](https://github.com/graphile/worker/commit/af85e51e9da62ebb811bc867eb2d5a827c01cd62) fix: fail_job function no longer exists (#289) | Use bulk SQL rather than removed fail_job during forceful shutdown. **Supported with Rust recovery policy.** Runner uses fail_job queries for handler errors and retirement-aware return on shutdown timeout. Shutdown and superseded_recovery tests cover ownership and attempts. |
| [0c797603e5ff79a71de56fb5e8363a7f8aba924f](https://github.com/graphile/worker/commit/0c797603e5ff79a71de56fb5e8363a7f8aba924f) fix: inconsistencies in SQL schema (#291) | Align add_job/reschedule signatures and job_spec types. **Supported.** SQL 13 records the old smallint step, SQL 14 restores public integer compatibility, and SQL 22 retains typed narrowing, keyed modes and retirement cleanup. |
| [bad84128d5bcd6c12a7aa38a556971f67c832ca0](https://github.com/graphile/worker/commit/bad84128d5bcd6c12a7aa38a556971f67c832ca0) 0.14.0-alpha.0 | Record package version/release metadata. **Inapplicable packaging.** Cargo pins and versions are independent. Runtime deltas have separate rows. |
| [56679986bee9238f32aa14c318f71fa359f03a50](https://github.com/graphile/worker/commit/56679986bee9238f32aa14c318f71fa359f03a50) sponsorship: update sponsors ❤️ (#308) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [9ac2c3943fda319f817b4bcb97f2d8c4f19c3266](https://github.com/graphile/worker/commit/9ac2c3943fda319f817b4bcb97f2d8c4f19c3266) feat: add Postgres version check (#309) | Require PostgreSQL 12 and check server version. **Supported.** pg_version.rs and install_schema protect both fresh and existing databases. |
| [77a6b0fd69f8425e3c113eb38c3261ecfe7e03a3](https://github.com/graphile/worker/commit/77a6b0fd69f8425e3c113eb38c3261ecfe7e03a3) Bump decode-uri-component from 0.2.0 to 0.2.2 (#306) | Update Node decode-uri-component. **Inapplicable.** Compiled task registration has no Node URL parser or shared npm graph. |
| [97b725eaa0706245c922037643cb0f8cf7d5a69e](https://github.com/graphile/worker/commit/97b725eaa0706245c922037643cb0f8cf7d5a69e) Bump json5 from 2.2.1 to 2.2.2 (#310) | Update Node json5. **Inapplicable dependency.** Rust crontab uses a Cargo JSON5 parser and its own parsing tests. |
| [72c9843c91b99dee8c3e6edf66b6e8157fc26f69](https://github.com/graphile/worker/commit/72c9843c91b99dee8c3e6edf66b6e8157fc26f69) fix: factor useNodeTime into prepared statement name (#300) | Distinguish local clock SQL in prepared statement identity. **Supported.** fetch_query_cache keys time/flag/query shapes separately and binds values. Upstream-sync tests vary time and flags. |
| [52ef94dc97b67a38e61452d1d422839ad9b7d8f9](https://github.com/graphile/worker/commit/52ef94dc97b67a38e61452d1d422839ad9b7d8f9) fix: unlocked jobs should have run_at bumped to now() (#313) | Raise recovered run_at to max(previous run_at, now). **Supported with Rust policy.** SQL 22 applies the greater of the original time and now plus configured delay for ordinary recovery. Retirement markers and attempt restoration are Rust extensions. See tests/worker_recovery. |
| [9a817c1f1903abdda33236a89b72461793389c41](https://github.com/graphile/worker/commit/9a817c1f1903abdda33236a89b72461793389c41) sponsorship: update sponsors 🙏 (#317) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [b3c1bace2def0b4d5615714302a30b9488676b65](https://github.com/graphile/worker/commit/b3c1bace2def0b4d5615714302a30b9488676b65) docs: fix typos (#316) | Fix prose typos in upstream docs. **Inapplicable copy.** Rust guides use independent wording. |
| [fb6b076d019c238b96f7bdf25725ca42bf118764](https://github.com/graphile/worker/commit/fb6b076d019c238b96f7bdf25725ca42bf118764) Change add_job signature back to using int instead of smallint (#325) | Restore integer arguments for public add_job. **Supported.** SQL 14, 16 and 22 expose integer max_attempts/priority and cast internally. Upgrade and add-job tests exercise current calls. |
| [5b9f85854967f33a82faef77221f478b384ef8a2](https://github.com/graphile/worker/commit/5b9f85854967f33a82faef77221f478b384ef8a2) 0.14.0-rc.0 | Record package version/release metadata. **Inapplicable packaging.** Cargo pins and versions are independent. Runtime deltas have separate rows. |
| [6fcfca5d9d65d6a031806519af203bff0970c375](https://github.com/graphile/worker/commit/6fcfca5d9d65d6a031806519af203bff0970c375) sponsorship: update sponsors 🙏 (#329) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [4d05832a1712f456d92001514c9474da73b56901](https://github.com/graphile/worker/commit/4d05832a1712f456d92001514c9474da73b56901) sponsorship: update sponsors 🙏 (#332) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [fc26c117bc057ec846d5c95375bd7bc1bee26e80](https://github.com/graphile/worker/commit/fc26c117bc057ec846d5c95375bd7bc1bee26e80) Bump word-wrap from 1.2.3 to 1.2.4 (#339) | Update npm word-wrap. **Inapplicable.** Upstream development lockfile and formatter dependency, separate from Cargo and the admin UI. |
| [5fb57d42f53929cda1e64838fff65da1ed3cc442](https://github.com/graphile/worker/commit/5fb57d42f53929cda1e64838fff65da1ed3cc442) sponsorship: update sponsors 🙏 (#341) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [ee89e3c3babdbec3fc656071b81aaa71fe5e3320](https://github.com/graphile/worker/commit/ee89e3c3babdbec3fc656071b81aaa71fe5e3320) Bump semver from 5.7.1 to 5.7.2 (#338) | Update npm semver. **Inapplicable.** Rust release API tooling uses cargo-semver-checks, with no shared npm lockfile. |
| [0db524974877ba2e962319b545d4180bdd236dfa](https://github.com/graphile/worker/commit/0db524974877ba2e962319b545d4180bdd236dfa) Bump tough-cookie from 4.0.0 to 4.1.3 (#336) | Update npm tough-cookie. **Inapplicable.** No Node cookie handling in Rust worker or sharing of this dependency change. |
| [b4972e8ef791211e8d06a3f1516699008c49c7df](https://github.com/graphile/worker/commit/b4972e8ef791211e8d06a3f1516699008c49c7df) docs: add some info regarding pg roles (#335) | Guide schema owner and restricted worker roles. **Supported.** Operations/migrations documents ownership and restricted roles, including startup access to an already current schema. |
| [4be366ae44875fc7b7006ed277d6b7ea51ba6151](https://github.com/graphile/worker/commit/4be366ae44875fc7b7006ed277d6b7ea51ba6151) sponsorship: update sponsors 🙏 (#344) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [53e0739b2674dcddf71ee27630bfb6cf8118f4ab](https://github.com/graphile/worker/commit/53e0739b2674dcddf71ee27630bfb6cf8118f4ab) feat: inline SQL scripts for bundlers (#345) | Bundle migration SQL instead of loading from disk while running. **Supported.** include_migrations embeds SQL at build time and registry tests verify ordering and metadata. |
| [2282dce7ee228dc393e6767354e10c250c0745c7](https://github.com/graphile/worker/commit/2282dce7ee228dc393e6767354e10c250c0745c7) chore: fixes to previous PR (#347) | Correct generated SQL/package setup after bundling. **Supported capability.** Packaged Rust SQL is proc-macro-generated and does not depend on Node prepack/generation. |
| [4138f75b98ec6ac4359596cbd7dae0aa48151756](https://github.com/graphile/worker/commit/4138f75b98ec6ac4359596cbd7dae0aa48151756) 0.14.0 | Record package version/release metadata. **Inapplicable packaging.** Cargo pins and versions are independent. Runtime deltas have separate rows. |
| [08ba506897917ffb9f85071ffa2357f2e81a6378](https://github.com/graphile/worker/commit/08ba506897917ffb9f85071ffa2357f2e81a6378) Add release notes for 0.15 | Release notes summarize the 0.15 database and runtime changes. **Reconciled.** See #274, #286 and #353 rows for the inherited behaviors. |
| [aae7527fe4a2e3464bb2f198a228c4219ffc6cd9](https://github.com/graphile/worker/commit/aae7527fe4a2e3464bb2f198a228c4219ffc6cd9) 0.15.0 | Record package version/release metadata. **Inapplicable packaging.** Cargo pins and versions are independent. Runtime deltas have separate rows. |
| [5ddf9de1b0ca5b26e95aba75a834abd77c03e9ee](https://github.com/graphile/worker/commit/5ddf9de1b0ca5b26e95aba75a834abd77c03e9ee) Bump postcss from 8.4.14 to 8.4.31 (#351) | Update PostCSS in Node build graph. **Inapplicable.** This is a different lockfile from the Rust admin UI. |
| [c02df91b5e54807f796b7cdb3f6a729907191e27](https://github.com/graphile/worker/commit/c02df91b5e54807f796b7cdb3f6a729907191e27) fix: gracefulShutdown is now graceful; introduce forcefulShutdown (#353) | Graceful shutdown drains active jobs, with separate forced shutdown and process signals. **Supported with Rust policy.** Explicit Worker shutdown, configurable grace and past-deadline claim returns. Shutdown tests exercise success and attempt-restoring interruption. Node numeric exit codes, signal repeat windows and runners’ Promise API are not copied. |
| [a2f8e0a09ca228e092b5ac5a9367f7db699e8634](https://github.com/graphile/worker/commit/a2f8e0a09ca228e092b5ac5a9367f7db699e8634) 0.15.1 | Record package version/release metadata. **Inapplicable packaging.** Cargo pins and versions are independent. Runtime deltas have separate rows. |
| [e3c59a6321350f8a82c5e70b82787d45471e2f7e](https://github.com/graphile/worker/commit/e3c59a6321350f8a82c5e70b82787d45471e2f7e) Bump @babel/traverse from 7.18.5 to 7.23.2 (#354) | Update Babel traverse. **Inapplicable.** Rust worker and mdBook have no Babel task transpilation. |
| [912df80b93fa44a11d51bf7169cf9819c94ec944](https://github.com/graphile/worker/commit/912df80b93fa44a11d51bf7169cf9819c94ec944) Integrate Graphile Config into Worker (#355) | Resolve Graphile Config presets, options and typed extension points. **Supported through Rust design.** WorkerOptions, typed plugins, extensions and driver options provide configuration without graphile.config, TypeScript globals or preset merge rules. |
| [3b95eff09723d80837923c446a32aca83f6579b7](https://github.com/graphile/worker/commit/3b95eff09723d80837923c446a32aca83f6579b7) Create documentation website (#356) | Introduce the Docusaurus website and document existing behavior. **Supported documentation capabilities.** mdBook covers configuration, tasks, cron, scheduling, SQL and operations. Rust has a separate documentation build and deployment. |
| [b46fe495ed53776bd52ccd68c9e87b255868206b](https://github.com/graphile/worker/commit/b46fe495ed53776bd52ccd68c9e87b255868206b) Add jobKey support to crontab (#294) | Cron job keys and replacement modes. **Supported.** CrontabOptions, CronBuilder and schedule.rs forward job_key/mode to SQL. Parser/typed and keyed job tests retain Rust snake-case options. |
| [f4a2c841b17726e84103ec1756b2c3e9c58d2093](https://github.com/graphile/worker/commit/f4a2c841b17726e84103ec1756b2c3e9c58d2093) Remove maxContiguousErrors (#357) | Remove arbitrary maxContiguousErrors from worker fetch loop. **Supported capability.** Rust exposes shutdown on fatal errors and opt-in SQL retry with original error results, without this threshold flag. |
| [551df64b850fb4c8a27a420b6f150aad865806ee](https://github.com/graphile/worker/commit/551df64b850fb4c8a27a420b6f150aad865806ee) Add Docker Hub action (#358) | Package Node CLI with a Docker Hub publishing workflow. **Inapplicable.** Rust has separate release workflows, and this audit has no release or publishing operation. |
| [0a8714a3bb28bc8cbc9f31789145a131d5f95abf](https://github.com/graphile/worker/commit/0a8714a3bb28bc8cbc9f31789145a131d5f95abf) Copy scripts into Docker | Include Node build scripts in container. **Inapplicable.** Rust embeds SQL and its worker has no Node scripts at runtime. |
| [6f78061b9f54e49a0de54a3898aa55a32f23c29b](https://github.com/graphile/worker/commit/6f78061b9f54e49a0de54a3898aa55a32f23c29b) Unregister signal handlers on worker shutdown (#359) | Release process signal handlers on worker stop. **Supported with Rust runtime policy.** shutdown_signal builds a selectable signal future and drops its subscriptions. Tokio installation is process-wide. Builder tests verify custom/manual signal behavior. |
| [52518a7dc496aff299666ce542fb0396be1a19ce](https://github.com/graphile/worker/commit/52518a7dc496aff299666ce542fb0396be1a19ce) Assert that database schema isn't more up to date than current worker (#360) | Refuse a newer incompatible schema during startup. **Supported.** Migration runner rejects newer breaking revisions and warns for compatible metadata. tests/migrate/future_revision.rs validates rejection. |
| [c409521741c9fcf6acdebabc3a7a3289a0726351](https://github.com/graphile/worker/commit/c409521741c9fcf6acdebabc3a7a3289a0726351) Detect migrations and shut down (#361) | Publish a migration notice and drain running consumers. **Implemented.** The active worker confirms the database revision in its own schema before stopping. See transactional notification and shutdown tests. |
| [b63785cb81385f0b0486ae35f852bedb06fc6326](https://github.com/graphile/worker/commit/b63785cb81385f0b0486ae35f852bedb06fc6326) Refactoring (#362) | Test job helper and type refactoring. **Inapplicable harness.** Rust fixtures use typed JobSpec and typed job queries. |
| [1febd3d113f161803ed2162f76d225a65516b84a](https://github.com/graphile/worker/commit/1febd3d113f161803ed2162f76d225a65516b84a) job1 -> job3 (#364) | Correct sample job reference in CLI docs. **Inapplicable copy.** Rust CLI uses a different example and argument syntax. |
| [ab760a2cb85036042148dd2b0e3e27c1bd722e35](https://github.com/graphile/worker/commit/ab760a2cb85036042148dd2b0e3e27c1bd722e35) Add more detail to cron:backfill event (#365) | Include persisted known_since/last_execution in backfill events. **Implemented.** Typed CronBackfill includes the selected entries and those timestamps before scheduling. |
| [6de84827db45a2c03ad88cab89fccd1906fb748a](https://github.com/graphile/worker/commit/6de84827db45a2c03ad88cab89fccd1906fb748a) Make tasks generic (#363) | Generic, typed tasks and typed job submission. **Supported.** TaskHandler, typed WorkerUtils submission and serde bounds preserve Rust unit/scalar payloads. |
| [a0672ba58bb7dd68f623a38d5b019ab41953dba0](https://github.com/graphile/worker/commit/a0672ba58bb7dd68f623a38d5b019ab41953dba0) Remove watch mode (#366) | Remove hot reload/watch of Node task files. **Inapplicable.** Rust task handlers are compiled and registered explicitly. |
| [0b6a22fd0859e4e00525737e2b5520bdeaf747dd](https://github.com/graphile/worker/commit/0b6a22fd0859e4e00525737e2b5520bdeaf747dd) Add support for importing ESM files (#367) | ESM/CommonJS dynamic loading. **Inapplicable.** Worker/CLI do not load JavaScript modules. |
| [0a92f2f100b6c16682b72cb74ad1395988f3b6e3](https://github.com/graphile/worker/commit/0a92f2f100b6c16682b72cb74ad1395988f3b6e3) Add number of attempts and max attempts to job logging (#239) | Expose consumed and maximum attempts in success/failure logs. **Implemented.** Structured tracing now records both fields, preserving task outcome and level. |
| [99b15438847a87532c122ac4ed8233a3245556e7](https://github.com/graphile/worker/commit/99b15438847a87532c122ac4ed8233a3245556e7) website: Editorial (#368) | Editorial task/readme/contribution content for the Node site. **Inapplicable copy.** Rust has independent task/extension and contributor instructions. |
| [52c0be1524709edf90e430547053780c45f73513](https://github.com/graphile/worker/commit/52c0be1524709edf90e430547053780c45f73513) website: Add a glossary page (#369) | Add worker, pool and job glossary. **Supported.** Rust mdBook concepts and architecture document concurrency, queues and payloads in Rust terminology. |
| [e95b76d98bebefe56c9140982137bf4fc8f7de2f](https://github.com/graphile/worker/commit/e95b76d98bebefe56c9140982137bf4fc8f7de2f) website: Add FAQ and Scaling pages (#370) | FAQ and scaling guidance. **Supported.** Worker, queue, LocalQueue and operations guides document concurrency and queue levels with Rust defaults. |
| [298d10f846a7a58f5f5706aaaf904fc2851df7a3](https://github.com/graphile/worker/commit/298d10f846a7a58f5f5706aaaf904fc2851df7a3) Add article explaining not to read from the 'jobs' table (#371) | Warn against depending on the private Jobs table. **Supported.** Rust compatibility and operations guide describe public functions/view and private table boundaries. |
| [60f0806c1214fedd68af0de5b209315cd9830981](https://github.com/graphile/worker/commit/60f0806c1214fedd68af0de5b209315cd9830981) Export LogLevel as a type (#372) | Export the JavaScript logger LogLevel type. **Inapplicable.** Rust uses the tracing level/subscriber API. |
| [6490c77525123418feda7bc08e52a199222c0933](https://github.com/graphile/worker/commit/6490c77525123418feda7bc08e52a199222c0933) Add a couple clarifications to the docs (#373) | Clarify administrative function and public-schema guidance. **Supported.** Rust compatibility and job-management docs cover force unlock, rescheduling and private table boundaries. |
| [b7f50a51167e315c96494c8fe9841c7b1fe8544f](https://github.com/graphile/worker/commit/b7f50a51167e315c96494c8fe9841c7b1fe8544f) Add forceUnlockWorkers API (#374) | Expose force_unlock_workers via SQL and utilities. **Supported.** SQL 15/16 and WorkerUtils pool/transaction methods, with force_unlock_workers integration coverage. |
| [55603eb0d71c3c8e560736c3137d0cf5b351b5a9](https://github.com/graphile/worker/commit/55603eb0d71c3c8e560736c3137d0cf5b351b5a9) Rename tables to make privacy clear (#375) | Rename private tables. **Supported.** Applied SQL 16 renames tables and updates SQL functions. Rust query identifiers use private table names. |
| [c49a68cf0c049050296f6e3fb99bb7ffe3d69a61](https://github.com/graphile/worker/commit/c49a68cf0c049050296f6e3fb99bb7ffe3d69a61) Add a 'jobs' view that people can select from (#376) | Expose the public jobs view without payload. **Supported.** SQL 17 and operations/compatibility docs, with migration and administration coverage. |
| [95b57d12f7cda1e328043e7ae9f1fe3530c7a68b](https://github.com/graphile/worker/commit/95b57d12f7cda1e328043e7ae9f1fe3530c7a68b) website: Editorial (#379) | Edit Node website copy. **Inapplicable.** mdBook pages follow Rust APIs, not the Node site. |
| [a33865d1718ce182d806cdc401fb8cfb4d1eecbb](https://github.com/graphile/worker/commit/a33865d1718ce182d806cdc401fb8cfb4d1eecbb) Fix job-key link | Repair a Node job-key docs link. **Inapplicable copy.** Rust scheduling guide has its own keyed scheduling section. |
| [3b409165243145bb41358519bd26605e68a435e9](https://github.com/graphile/worker/commit/3b409165243145bb41358519bd26605e68a435e9) Load tasks from subfolders; support .cjs/.mjs/binaries; pluggable task loading (#378) | Load nested tasks, ESM/CJS and executable files with pluggable loaders. **Inapplicable loaders.** Rust compiled TaskHandler supports static slash identifiers and separate explicit task registration. No child-process protocol. |
| [498103eb3abbf01f74a0425b25b2c4228957022f](https://github.com/graphile/worker/commit/498103eb3abbf01f74a0425b25b2c4228957022f) Add Algolia DocSearch | Add hosted DocSearch. **Inapplicable site packaging.** mdBook has its own search. |
| [13dc8ce2b4897f4f0c05f7d95ccd789ac87af514](https://github.com/graphile/worker/commit/13dc8ce2b4897f4f0c05f7d95ccd789ac87af514) sponsorship: update sponsors 🙏 (#382) | Update upstream sponsorship copy. **Inapplicable.** The Rust maintainer has independent funding metadata. |
| [5732652a41bdca22159f74470f987e54d8db027c](https://github.com/graphile/worker/commit/5732652a41bdca22159f74470f987e54d8db027c) fix `docker-compose up -d` (#384) | Fix command spelling for Node Docker Compose docs. **Inapplicable copy.** Rust uses just test-docker and PostgreSQL services. |
| [aa8ac89041e4c88d29d2664a703d7b249fe13c30](https://github.com/graphile/worker/commit/aa8ac89041e4c88d29d2664a703d7b249fe13c30) Fix link to worker utils (#383) | Fix Node WorkerUtils docs link. **Inapplicable copy.** Rust documentation has independent WorkerUtils links and signatures. |
| [3acfb342230c7ae3420c0d1dd49699d48cde87c5](https://github.com/graphile/worker/commit/3acfb342230c7ae3420c0d1dd49699d48cde87c5) Overhaul plugin hooks system and fix various lifecycle issues (#386) | Overhaul lifecycle, on-the-fly breaking flags, concurrent migration clashes, cancellation and command loading. **Supported/implemented.** Typed lifecycle hooks and extension handles replace most Node event/preset objects. Rust already owns graceful drain and compatibility checks. Added legacy flag repair. Pre/post migration interception and bootstrap hooks remain outstanding, described below. |
| [8ed836c7c4aeb9d1f3a44b81f8b2827ee24a9078](https://github.com/graphile/worker/commit/8ed836c7c4aeb9d1f3a44b81f8b2827ee24a9078) When queuing multiple jobs at once, nudge more than one worker (#389) | Send batch size in notifications and nudge enough idle workers. **Supported with Rust fanout.** SQL 18 and 22 send counts. dispatch.rs fans out bounded hints up to concurrency, including for older empty payloads. It does not use payload count as an upper bound. Existing batch and queue-wakeup tests exercise bounded fanout and locks. |
| [50752a2513914802eee7bd3ca41ecf6c9130fe1b](https://github.com/graphile/worker/commit/50752a2513914802eee7bd3ca41ecf6c9130fe1b) Reduce calls to processSharedOptions by passing pre-processed values more consistently (#390) | Share compiled option processing. **Inapplicable refactor.** Rust constructs WorkerOptions then moves typed configuration once. JS preset resolution does not run during claims. |
| [eebab6c267c7dcb38d7b0815a4f24296148a9460](https://github.com/graphile/worker/commit/eebab6c267c7dcb38d7b0815a4f24296148a9460) Forward-port fixes from back-port of plugin system (#393) | Move notification to worker:migrate, forward bootstrap/migrate hooks and stop ordering. **Implemented/supported in part.** Current watcher uses worker:migrate. Shared shutdown and awaited cleanup preserve lifecycle. Custom migration hooks remain outstanding. |
| [ce655b9f18c8763feae9c9bf1b2ecba15ef9139c](https://github.com/graphile/worker/commit/ce655b9f18c8763feae9c9bf1b2ecba15ef9139c) website: Website Refresh (#394) | Refresh upstream website themes/navigation. **Inapplicable site-specific changes.** Rust mdBook is independent. |
| [1b62965f9217efe0335cc48246ca88d09f2b7634](https://github.com/graphile/worker/commit/1b62965f9217efe0335cc48246ca88d09f2b7634) Forward-port back-port fixes (#395) | Forward backports: clock-consistent stale reset, prepared options, guard empty task list, shared runner/preset changes. **Supported with Rust design.** Time-shaped cached SQL, typed options, no task means no eligible claim, safe dispatch drains, and recovery configurable delays. Node CLI deprecation and JS configuration copy do not apply. |
| [1d378c5c92483d20a84a6c6b10f08117de866fb0](https://github.com/graphile/worker/commit/1d378c5c92483d20a84a6c6b10f08117de866fb0) Add random number to notify payload to prevent deduplication (#391) | Randomize NOTIFY payload to prevent transaction coalescing of separate inserts. **Supported.** SQL 18, 21 and 22 use random() and count for insert notifications. |
| [c34cd15b606f13599c074624209e642ed5dd766c](https://github.com/graphile/worker/commit/c34cd15b606f13599c074624209e642ed5dd766c) 0.16.0-rc.0 | Record package version/release metadata. **Inapplicable packaging.** Cargo pins and versions are independent. Runtime deltas have separate rows. |
| [2c8091e0bcce5d39e46a1b3833daf20b59097d1e](https://github.com/graphile/worker/commit/2c8091e0bcce5d39e46a1b3833daf20b59097d1e) Worker Pro documentation (#396) | Document proprietary Worker Pro migration coordination. **Inapplicable proprietary protocol.** Rust has its own heartbeats, opt-in recovery and post-commit backstop. |
| [6bfaf52997c3e2a7173b9d7c70f117159cf3a437](https://github.com/graphile/worker/commit/6bfaf52997c3e2a7173b9d7c70f117159cf3a437) Merge branch '0.13.x' | Merge 0.13.x bridge history. **Reconciled.** Its main-parent delta is README bridge guidance, with no branch-only commits relative to current main. Bridge workers/license and Node plugins are outside the Rust runtime. |
| [145dd287f585fb7c7460cf18247d4cc72f9b144d](https://github.com/graphile/worker/commit/145dd287f585fb7c7460cf18247d4cc72f9b144d) Merge branch '0.15.x' | Merge 0.15.x bridge history. **Reconciled.** Main-parent delta is release notes and Yarn lockfile. See #353/#386/#393/#395 for forward-ported runtime behavior. No current remote maintenance delta outside main. |
| [a795f55c8cf4d218999be54c5d3c6f1e825e8a01](https://github.com/graphile/worker/commit/a795f55c8cf4d218999be54c5d3c6f1e825e8a01) Document that you must clean up your database when you adopt pro (#399) | Document clearing incompatible state before Worker Pro adoption. **Inapplicable vendor setup.** Rust has its own public utilities and operational migration rules. |
| [46e3a282b4efc24beaa91004fa9fa7335202cf5d](https://github.com/graphile/worker/commit/46e3a282b4efc24beaa91004fa9fa7335202cf5d) Pro: document worker recovery features (#400) | Document Worker Pro recovery. **Inapplicable proprietary coordinator.** Rust recovery guide covers heartbeats, attempt restoration, flags and retirement markers with its own behavior. |
| [1f6d9bd9784f3573e30239005263aa501250d46e](https://github.com/graphile/worker/commit/1f6d9bd9784f3573e30239005263aa501250d46e) Add to release notes comment about Worker Pro migrations | Add release note about pre-upgrade Worker Pro coordination. **Inapplicable vendor setup.** Rust docs describe stopping workers and post-commit detection separately. |
| [a6c32158a8d71b4e213fea867e631c914d41e0f7](https://github.com/graphile/worker/commit/a6c32158a8d71b4e213fea867e631c914d41e0f7) Use a custom error message for migration 11 error (#401) | Report an actionable error when locked jobs block migration 11. **Supported.** MigrateError::LockedJobInMigration11 on 22012 and locked_jobs_migration_11 regression. |
| [14765437e3f3821863628197b71cde7a99c26d23](https://github.com/graphile/worker/commit/14765437e3f3821863628197b71cde7a99c26d23) Tweak version script | Adjust versioning script. **Inapplicable packaging.** Rust releases use release-plz. |
| [894d9429d4f90cee5e248b84b0736308d19696b9](https://github.com/graphile/worker/commit/894d9429d4f90cee5e248b84b0736308d19696b9) 0.16.0-rc.1 | Record package version/release metadata. **Inapplicable packaging.** Cargo pins and versions are independent. Runtime deltas have separate rows. |
| [a5af1761b5f4800942c00c3373b5c6903de43594](https://github.com/graphile/worker/commit/a5af1761b5f4800942c00c3373b5c6903de43594) perfTest example: move python validation earlier (#402) | Validate Python earlier in Node perf harness. **Inapplicable harness.** Rust benchmark entrypoints use Cargo and the isolated PostgreSQL recipe. |
| [4e9a886efe4bdf22836e1e9252bae231952c34e2](https://github.com/graphile/worker/commit/4e9a886efe4bdf22836e1e9252bae231952c34e2) website: Docs tweaks to AddJob and TypeScript pages (#404) | Clarify typed producer and scheduling examples. **Supported Rust guidance.** TaskHandler, JobSpecBuilder and SQL scheduling guides describe corresponding contracts. |
| [13d98b773a48b448d3577e2bff006759c335b276](https://github.com/graphile/worker/commit/13d98b773a48b448d3577e2bff006759c335b276) website: document unix domain socket connection for google cloud SQL (#410) | Document Cloud SQL Unix sockets and database URLs. **Driver-specific.** Node socket: and schemaless forms do not apply to Rust. Rust can take configured native driver/pool handles. No live Cloud SQL Proxy assessment. |
| [bf0593a32b0c2a47343bea734cbd455e17a49ff4](https://github.com/graphile/worker/commit/bf0593a32b0c2a47343bea734cbd455e17a49ff4) website: News site (#405) | Introduce Node website news. **Inapplicable editorial feature.** Rust has separate mdBook/release history. |
| [0ffa9ce6b25c9e294980099158835d6b65cff4a9](https://github.com/graphile/worker/commit/0ffa9ce6b25c9e294980099158835d6b65cff4a9) Fix links | Correct links in website news content. **Inapplicable.** Those news pages are absent from Rust. |
| [02056135bb4f01eeef284bdbc28360eb0f93cba6](https://github.com/graphile/worker/commit/02056135bb4f01eeef284bdbc28360eb0f93cba6) website: News date fix (#411) | Correct website news date display. **Inapplicable.** No shared news renderer. |
| [3e4b5180ce4d55860280978a9e438d6d0110d0fb](https://github.com/graphile/worker/commit/3e4b5180ce4d55860280978a9e438d6d0110d0fb) Fix documentation on maxAttempts to match definition (#413) | Correct maximum attempts documentation. **Supported.** Rust JobSpec and scheduling guide define the total attempt limit. |
| [188c330e76e036bf2671e772c9b86b93beb5b31b](https://github.com/graphile/worker/commit/188c330e76e036bf2671e772c9b86b93beb5b31b) 0.16.0 | Record package version/release metadata. **Inapplicable packaging.** Cargo pins and versions are independent. Runtime deltas have separate rows. |

## Tracking proposals

The 12 open PRs and their heads are unchanged from the
[November 12 row-by-row proposal ledger](graphile-worker-js-upstream-2026-11-12.md#open-and-closed-proposals).
#633 is explicit migration entrypoints, #637 is task-cache recovery, #642 is
sponsorship, #643 is direct outcome errors, #644 is keyed serialization, #645
is batch-loop retries and reset, #646 is keyed/array retirement, #647 is Node
dependencies, #648 is runner errors, #649 is LocalQueue documentation, #654
is the proposed 0.18.1 release, and #655 is Node task-file URL handling. The
prior closed and unmerged #640 and #651 remain closed. No proposal or
synthetic merge ref was counted as merged behavior.

## Remaining coverage and compatibility

1. **Pre-v0.13.0 source history.** No recovered September 5 report establishes
   comprehensive coverage there. The audit includes the targeted #163 cron
   check but does not certify that entire interval. Reconstruct the earlier
   release lineage and first-parent behavior ledger to extend coverage.
2. **Bootstrap and migrate interception from #386/#393.** Rust has an
   initialized-worker observer and explicit migration calls. It still lacks a
   typed equivalent of Node's pre/post bootstrap and migration callbacks with
   mutable access to the migration client and a failure callback after rollback.
   In the pinned Node fixture, all four callbacks used one backend PID, and
   a temporary table created before migrations remained visible after them.
   Rust's `DatabaseDriver` exposes pooled statements and owned transactions,
   and `runner.rs` opens a transaction per revision. A pooled observer cannot
   preserve temporary tables or session settings from one phase to the next.
   The unblock condition is a portable pinned-session executor with explicit
   transaction boundaries, a rollback-before-error-hook guarantee and coverage
   through both the standalone migration and WorkerOptions paths. This remains
   an implementation gap and the inspected SHA does not close it.
3. **Cron schedule and payload compatibility.** Rust preserves its existing
   matching of both restricted day fields, historical weekday step offsets,
   and typed non-object payloads. Upstream's OR rule can add executions to an
   existing deployed schedule, and wrapping scalar payloads breaks typed
   deserialization. ADR 0007 documents per-entry predicate opt-in. A separate
   upstream-format compatibility mode and an explicit payload envelope contract
   would be needed before changing those defaults.
4. **#645 and #646 remain open.** Reassess their final merged behavior or a
   separately reproduced Rust defect. Asynchronous batch outcome errors and
   reset/backpressure policy differ. Rust retains retirement markers and
   advisory lock ordering. A mixed-runtime producer/retirement guarantee needs
   a shared lock/ownership protocol and a two-runtime recovery test.
5. **Historical retired rows.** Existing pre-22 rows still cannot distinguish
   a retired keyed job from an ordinary exhausted job. Reliable historical
   evidence or an explicit operator selection would be necessary to backfill
   markers. The new migration-ledger repair fills only known breaking flags.
6. **Operational scope.** PostgreSQL 12 compatibility remains documented.
   A full PG12 and cross-OS rerun, Cloud SQL Proxy and every mixed-runtime
   schema or recovery combination are not certified. Rust and Node use
   different revision histories. #655's loader is absent from Rust and
   #647's Node dependency graph is independent. The withdrawn braces advisory
   GHSA-vfj7-8cjw-p6xm still reports withdrawal on 2026-10-23. ADR 0004's
   requirements/capabilities protocol remains a product decision.

## Validation

Rust 1.99.1 and PostgreSQL 18.7 were used locally. The regression suite exercises
migration emission after commit, rejection of future breaking revisions,
ignored rollback/nonbreaking and unrelated-schema notifications, draining
with a handler in flight in direct and LocalQueue modes, legacy revision-13
ledger upgrades retaining a real job, and fresh/Node and Rust revision-20
upgrade paths. The cron mock-clock test verifies backfill at minutes 2 and 4,
the predicate's live tick at 6, and an ordinary timer at 7, with persisted
history fields and no extra jobs. Parser tests cover duplicate IDs and the two
Sunday aliases. A separate mock-clock test injects a one-time matcher panic
and verifies backoff, backfill and resumed scheduling. Another regression
captures attempts and limits in structured success and failure logs.

The pinned published Node v0.18.0 package, verified by npm gitHead, was
run with Node 24.22.0 against separate UUID PostgreSQL 18.7 fixtures. It
emitted 20 notices for 20 revisions. A rolled-back migration left the worker
running, while a committed breaking migration let its active handler finish
and set Node exit code 57. The equivalent Rust regression drains and returns
an incompatible-schema error. Under the same historical cron clock and seeded
known-since/last-execution, Node and Rust both backfill minutes 2 and 4 with
matching timestamps. Both reject duplicate text identifiers and match Sunday
with `0`, `7` and `0-2`. Rust's extra unrelated-schema guard is intentional.
Both migration runners upgraded their revision-13 fixtures without losing the
queued job. Node reached revision 20 and Rust reached revision 22. Their
breaking revision histories remain runtime-specific. The tests exercise correctness
without measuring throughput.

On baseline `757ced2`, the new migration tests reproduced both notification
timeouts and SQLSTATE 42703 for the missing ledger column. These tests pass with
the port. The first cron test used a payload-free entry and exposed an existing
documented-shape mismatch. The fixture now supplies an empty object to test
metadata, and the guide explains how Rust handles non-object payloads.

Local delivery evidence is recorded below. Exact-head hosted Check, Test,
Coverage and Docs are delivery gates. Main results are verified at the actual
merge SHA and recorded in durable run output.

| Local check | Final result |
| --- | --- |
| `just lint` | Passed formatting, all-target checking and Clippy with warnings denied |
| `just test-docker` | PostgreSQL 18.7, 545 passed, 0 failed, 7 ignored across 91 suites. The recipe cleaned up its container. |
| PostgreSQL 12.22 with SQLx | 33 migration, cron and attempt-logging tests passed |
| PostgreSQL 18.7 with tokio-postgres and the rustls feature | 32 migration, cron and attempt-logging tests passed. The isolated database connection for this run was plaintext. Hosted TLS-only checks exercise the negotiated protocols. |
| Documentation and CI support scripts | mdBook 0.4.52 build, 14 release-tool tests, 6 TLS dependency isolation scenarios and the admin UI source-map regression passed. The npm audit for the locked admin UI reported zero vulnerabilities. |
| Schema and source verification | 253 cumulative first-parent deltas matched individually through earlier and current ledger rows. The seven prior downstream port SHAs are ancestors of current main. Packaged migration SQL 1–22 and MIT attribution are unchanged. |

The local test resource settings used three build jobs, one test thread and
disabled debug symbols for the final full workspace validation. All assertions
and CI configurations were retained.
