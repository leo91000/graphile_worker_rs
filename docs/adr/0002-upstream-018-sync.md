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

A PostgreSQL 16/17/18 test matrix is prepared following upstream, but the
existing OAuth connection cannot push workflow changes. The matrix patch is
preserved separately; the library-port branch retains PostgreSQL 18 CI. Node
version and Jest OS matrices do not transfer to Rust; existing Rust binary
targets and driver coverage remain in place. See the audit ledger for the
precise blocker and unblock condition.
