# SQL retries and the reconstructed 0.16 audit

The October 2026 audit broadens the previously reconstructed comparison from
upstream v0.16.6 back to v0.16.0. Earlier reports did not establish coverage of
that interval. This finds three missing public capabilities: SQL retry policy,
queue-name lookup, and additional task identifiers to preserve during cleanup.

## SQL retries

`SqlRetryPolicy` ports Graphile Worker's bounded retry policy, including the subsequent v0.17 exponential backoff update in #474. Its default
limit is 100 total attempts. Serialization failures (40001) and deadlocks
(40P01) start with 50ms exponential backoff, multiplying by 1.5 per failure and capping
at 5s. A server
that cannot connect now (57P03), host-unreachable errors, and connection timeouts
start at 3s and cap at 120s. Both receive a random 0.5–1.5 multiplier after
capping to avoid synchronized retries. Exhaustion returns the last error with its diagnostic
and code, without sleeping after the last attempt. Dropping the operation also
drops its backoff and prevents further attempts.

Retries are **opt-in** through `WorkerOptions::sql_retry_policy` or
`Database::with_retry_policy`. The existing immediate-error default remains:
applications may use these Results to decide their own retry/shutdown policy.
This differs from Node's automatic internal-query retries. When enabled, the
policy applies to that Database handle's pooled execute/fetch operations,
including worker claims, registration, direct/batched outcomes, context helpers
and utilities. Existing clones, raw pool/driver access and LISTEN do not acquire
that policy. Downcasting and prepared-statement configuration are preserved.

Transactions returned by `begin()` never replay statements or commit. PostgreSQL
serialization recovery requires restarting the entire transaction, including
application logic; replaying a statement inside an aborted transaction is
incorrect. The caller owns that retry. Custom DatabaseDriver implementations
must provide autocommit operations before enabling this policy. Drivers backed
by a caller's live transaction must leave retries disabled.

Connection I/O classification happens specifically at pool acquisition, before
SQL can be sent. Execution-time I/O errors have an uncertain commit outcome and
are never automatically replayed, even when their text or code resembles a
connection timeout. No message-string matching is used. Database-driver
conversion preserves SQLSTATEs, including a server rejection during acquisition.
A retry policy may extend time spent persisting an outcome during shutdown;
choose the attempt limit for the application's shutdown budget. The bound is
per statement: LocalQueue returns and cron also have outer retries, so their
limits compose. Cancellation
cannot undo already committed database work or nontransactional side effects
such as sequence advancement.

This is a Rust implementation of the policy in upstream `src/lib.ts`, commits
4e44699d8f72113f37ba8089776a859b57d76d2b (#430),
86bfcc841a06e052b48d1c2464d70080e4b0e720 (#439),
93e6e607f8bb37bdabc4faa44c62001b062480c4 (#445), and
3fa9f068069d131cc42f82427177c0b7155af668 (#451), and
5a305b9424ddcee44ef38cc4aa9181bb756cc73b (#474). Upstream MIT notices are
retained in the database crate license.

## Queue names and cleanup

`WorkerContextQueueExt::get_queue_name()` resolves the running job's name and returns
None for unnamed or missing queues; database failures remain Results. The query
crate's `get_queue_names` resolves arbitrary IDs in input order, preserving
missing entries and duplicates. The new WorkerContextQueueExt trait avoids adding required methods to existing
WorkerContextExt implementations. Rust deliberately performs an immediate query
rather than retaining Node's deferred batch/cache protocol. This avoids stale
names across administrative cleanup without introducing a new timer or shared
unbounded cache. No performance equivalence is claimed.

`WorkerUtils::cleanup_with_keep` combines explicit keep identifiers with the
worker's registered identifiers without adding those extra names to its task
cache or registering missing names. `cleanup` retains its previous behavior.
The administration CLI accepts repeated/comma-separated `--keep-task` values;
it has no JavaScript task-directory discovery. Existing cleanup locking,
READ COMMITTED snapshots, lock timeout and guarded cache refresh remain intact.
Other workers whose cached names are not preserved must still be stopped before
task GC. A keep list applies to one cleanup call only.

## Run-once errors and schema compatibility

An independent comparison with proposed upstream runner error handling found
that Rust's run_once fetch stream and named-queue continuation discarded errors.
It now retains the first fetch or persistence error while draining active jobs.
The existing public job streams keep their Job-only API; the worker uses an
internal fallible stream that yields a fetch error once and then terminates.
No new public error variants or signatures replace existing ones.

No migration SQL, private row type, PostgreSQL minimum, locking order, retry
attempt accounting, job-key retirement or mixed-worker guarantee changes.
Applied migrations 1–22 must remain byte-for-byte unchanged. This audit does
not import the migration in open upstream proposal #646: its locked-array retry
behavior is not merged upstream and differs from Rust's documented retirement
contract. Final merged behavior will need a fresh compatibility decision.
