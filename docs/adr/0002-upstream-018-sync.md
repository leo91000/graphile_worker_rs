# Upstream 0.18 synchronization

The September 2026 audit ports the named-queue batch-fetch fix and identity
allocation fix from Graphile Worker 0.18.0. Candidates are locked before
selecting one per named queue; unnamed jobs remain independent. The batch
limit applies to candidates, so deduplication may produce a smaller batch.

Migration 21 evolves the latest Rust SQL instead of replacing it with upstream
migration 20. Rust's sorted advisory locks for job keys and its missing-result
error remain in place. Applied migrations 1–20 are unchanged. Since Rust and
Node previously used revision 20 for different purposes, migration 21 also
installs recovery objects if absent and preserves existing recovery rows.

Fetch SQL is cached by escaped schema, presence of flags, time mode, and fetch
shape. Batch size and all job-specific values remain bind parameters. The
cache is per thread and capped at 128 entries, avoiding cross-thread locking
and unbounded retention in applications that use many schemas. This caches SQL
text only; it does not change driver statement caching or store job results.
Database benchmarks are recorded in the audit report without a throughput claim.

The older upstream safe-event-emission change also applies to Rust observers.
Observer construction and polling panics are caught and logged, and concurrent
observers and cleanup continue. Interceptors keep their existing control-flow
contract. This does not contain an aborting panic or promise transaction rollback
for application side effects made by a failing observer.

The runtime APIs remain Rust-native: no JavaScript module loaders, EventEmitter
objects, graphile.config files, or ECMAScript async-disposal protocol are added.

The reconstructed audit also found that SQLx did not expose upstream's opt-out
from persistent named statements. `SqlxDatabase::with_prepared_statements(false)`
now configures both direct queries and transactions created by that wrapper.
Defaults and raw SQLx executors remain unchanged; configure a fresh pool before
use, since SQLx can reuse statements previously cached by other callers.

A PostgreSQL 14/15/16/17/18 test matrix is prepared following upstream, but the
existing OAuth connection cannot push workflow changes. The matrix patch is
preserved separately; the library-port branch retains PostgreSQL 18 CI. Node
version and Jest OS matrices do not transfer to Rust; existing Rust binary
targets and driver coverage remain in place. See the audit ledger for the
precise blocker and unblock condition.

## Replaced jobs and Rust recovery

Review identified an interaction with Rust's attempt-restoring recovery: upstream
retires a replaced locked job by clearing its key and exhausting its attempts.
Decrementing attempts on recovery would make that obsolete payload runnable again.
Simply clearing ownership during replacement would break the running handler's
failure/return path and could leave its named queue locked.

Migration 22 therefore adds a private `_private_job_retirements` table. Replacement sets
it while retaining ownership; dead-worker recovery, shutdown and local-queue
return release locks without restoring an attempt for marked jobs. Ordinary
final attempts still restore their consumed attempt. The public reschedule_jobs
function clears the marker so explicitly rescheduled jobs recover normally.
Applied migrations 1–21 are not rewritten. Existing rows remain unmarked:
the migration cannot infer whether an older exhausted locked row was replaced
or is an ordinary final attempt. This fix governs replacements performed after
migration 22; it does not reconstruct historical replacement events.

The marker is kept in a separate table with an ON DELETE CASCADE foreign key,
so job deletion removes it. The private job composite row type stays unchanged: an
initial column-based design failed the existing upgrade test with PostgreSQL
"cached plan must not change result type". The retained upgrade assertions
pass with the separate table; no pool reconnect or assertion removal is needed.

The same marker also protects jobs explicitly retired by remove_job or
permanently_fail_jobs. These operations likewise must not be undone by automatic
attempt restoration. Their existing lock-age restrictions remain unchanged.

All attempt-restoring operations lock their job rows before reading retirement
markers in a subsequent statement. At READ COMMITTED, that subsequent statement
gets a fresh snapshot and sees a retirement committed while the row lock was
being acquired. The Rust return paths share `_private_return_jobs`; dead-worker
recovery follows the same locking order. A single UPDATE with a marker subquery
was experimentally shown to revive an obsolete row when replacement committed
while it waited. The concurrency regression waits for PostgreSQL to report the
blocked row lock, commits replacement, then checks all three return paths.

Explicit rescheduling uses the same lock-before-marker-read sequence. A controlled
concurrent permanently_fail_jobs/reschedule_jobs reproduction returned a revived
job with its retirement marker still present when both operations used a single
statement snapshot. The regression now verifies that rescheduling waits, clears
the committed marker, and permits ordinary attempt restoration on the next run.
