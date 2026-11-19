# Migration notices and programmatic cron

The historical upstream audit found live breaking-migration detection
([#361](https://github.com/graphile/worker/pull/361),
[#386](https://github.com/graphile/worker/pull/386),
[#393](https://github.com/graphile/worker/pull/393)), programmatic cron matchers
([#272](https://github.com/graphile/worker/pull/272)), and backfill history
events ([#365](https://github.com/graphile/worker/pull/365)).

## Migration compatibility

The migration runner repairs pre-breaking-flag ledgers on discovery of the
missing column, then rereads the ledger. It adds the column and annotates known
breaking revisions in a transaction. Existing revision IDs, timestamps,
migration SQL and private row types are retained. Revision numbers are local
to each runtime's migration history, so this does not reconcile arbitrary
mixed-runtime migration histories.

The runner emits the existing upstream `worker:migrate` payload, containing
`migrationNumber` and `breaking`, after executing SQL in the same transaction.
A rolled-back migration cannot announce itself. The built-in drivers listen on
both relevant channels using one connection. A notice is a hint. The worker
queries its own schema before reacting because PostgreSQL notifications are
database-wide. The worker also checks on its first poll and then on polling
ticks at least 30 seconds apart. This covers a missed notification, disabled
delivery and a driver implementing only the existing single-channel API.

On confirmed incompatibility, the dispatcher signals shutdown and waits for
in-flight handlers and LocalQueue cleanup. The worker returns a typed runtime
error after shutdown. Node's process exit code is specific to its CLI.
Detection occurs after a revision commits and therefore does not replace
stopping older workers before a breaking change.

## Cron compatibility

`WorkerOptions::with_cron_matcher` adds an entry with a synchronous Rust
predicate in place of its calendar timer. Stable identity, options, typed
payload and lifecycle hook values still come from the `Crontab`. The predicate
is stored with its entry position to preserve the existing public struct
and getter. It receives the same UTC or opt-in local calendar minute on live
scheduling, catch-up and backfill and should be deterministic and free of side
effects. Unwinding panics enter the existing cron backoff and registration
recovery path.

`CronBackfill` observes selected entries and their persisted history before
scheduling. Retried or concurrent schedulers may emit observations for the
same minute. PostgreSQL's registration and execution guard still controls
which jobs are actually inserted.

Text crontabs reject duplicate default or explicit identifiers. Weekday
values 0 and 7 both select Sunday. Existing typed weekday stepping and the
conjunction of the day-of-month and weekday restrictions are preserved to
avoid changing deployed job schedules. An application that intentionally
wants different matching rules can supply a predicate for that entry.
Absent, scalar and array cron payloads retain their Rust meaning, since typed
task handlers can deserialize them. Object payloads carry `_cron` metadata.

No existing migration file or database compatibility minimum changes. The
repository's MIT attribution is retained.
