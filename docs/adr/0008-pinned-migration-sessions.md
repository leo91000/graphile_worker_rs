# Pin the PostgreSQL session for migration callbacks

Graphile Worker [#386](https://github.com/graphile/worker/pull/386) and
[#393](https://github.com/graphile/worker/pull/393) expose callbacks before
and after schema bootstrap and migration, plus a callback after a failed
revision rolls back. The application can configure its migration session
and inspect it before continuing.

Rust exposes fallible callbacks through `MigrationHooks`, available with
worker initialization, standalone worker utilities and the migration crate.
Each registered callback receives the same session, schema, server version and
typed per-run scratchpad. The phase identifies the failed revision and carries
its original diagnostic when applicable. A callback can stop a migration by
returning an error. Transaction boundaries belong to the migrator, including
explicit rollback before a failure callback. A failed rollback aborts the run
and prevents the callback from receiving an unusable session. PostgreSQL
commits each revision and its notification together. A pre/post hook can have
its own committed effects even when the overall run returns an error.

Build-time and runtime ownership of application resources remain Rust-native.
The callback API gives extension code a borrowed executor and no continuation
that can skip the migration ledger or compatibility check. The JavaScript
middleware wrapper and mutable compiled options are specific to the upstream
plugin API. A direct connection or session-mode pool preserves temporary tables
and session settings across revision boundaries. Transaction-mode proxies
cannot provide that guarantee.

Opted-in migrations hold one pool lease for the whole run. SQLx closes the
lease on drop. The tokio-postgres driver detaches and drops the checked-out
client. This discards session changes on success, failure and cancellation.
An empty registry keeps the current pooled path, and custom drivers can opt
into the session contract without implementing a new required method.
Per-statement pooled retry policy does not replay pinned SQL. Existing
migration SQL, revision numbers, notices and PostgreSQL minimum remain intact.
