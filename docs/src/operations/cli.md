# CLI

The `graphile-worker` binary provides operational access to a Graphile Worker
PostgreSQL database. It can run migrations, add and inspect jobs, mutate job
state, run maintenance tasks, print queue and worker summaries, and serve the
embedded admin UI.

Most commands mutate the configured schema directly. Use the same database URL
and schema that your workers use.

## Connection Options

Every command accepts these global options:

```bash
graphile-worker \
  --database-url postgres://postgres:postgres@localhost/postgres \
  --schema graphile_worker \
  --max-connections 5 \
  --json \
  stats
```

| Option | Default | Description |
| --- | --- | --- |
| `--database-url` | `DATABASE_URL` | PostgreSQL connection URL. The CLI exits if neither is set. |
| `--schema` | `GRAPHILE_WORKER_SCHEMA`, then `graphile_worker` | Graphile Worker schema name. |
| `--max-connections` | `5` | Maximum PostgreSQL connections used by the CLI pool. |
| `--json` | disabled | Print machine-readable JSON where the command supports it. |

## Migrations

Run migrations before using a fresh database:

```bash
graphile-worker --database-url "$DATABASE_URL" migrate
```

With JSON output:

```bash
graphile-worker --database-url "$DATABASE_URL" --json migrate
```

The JSON form prints:

```json
{
  "migrated": true
}
```

## Add Jobs

Add a job by task identifier. If no payload is provided, the payload defaults to
an empty JSON object.

```bash
graphile-worker add send_email
```

Pass JSON directly:

```bash
graphile-worker add send_email \
  --payload '{"to":"user@example.com"}'
```

Read JSON from a file:

```bash
graphile-worker add send_email \
  --payload-file payload.json
```

Set common scheduling and routing fields:

```bash
graphile-worker add send_email \
  --payload '{"to":"user@example.com"}' \
  --queue emails \
  --run-at 2026-01-02T03:04:05Z \
  --max-attempts 5 \
  --priority 10 \
  --flag transactional
```

`--run-at` accepts an RFC 3339 timestamp or `now`. Lower priority values run
sooner.

Use job keys for deduplication or replacement:

```bash
graphile-worker add send_email \
  --key user-123-welcome \
  --job-key-mode replace
```

`--job-key-mode` requires `--key`. Supported modes are:

| Mode | CLI value |
| --- | --- |
| Replace | `replace` |
| Preserve run time | `preserve-run-at` |
| Unsafe dedupe | `unsafe-dedupe` |

`--payload` and `--payload-file` cannot be used together. Payload input must be
valid JSON.

## Inspect Jobs

List jobs:

```bash
graphile-worker list
```

Filter by state, task, queue, and pagination:

```bash
graphile-worker list \
  --state ready \
  --identifier send_email \
  --queue emails \
  --limit 25 \
  --offset 0
```

Supported states are `all`, `ready`, `scheduled`, `locked`, and `failed`.
`--limit` and `--offset` must be greater than or equal to zero.

Show one job by id:

```bash
graphile-worker show 123
```

Human-readable `list` output is tab-separated. Human-readable `show` output
includes the job payload pretty-printed as JSON. Add `--json` to either command
for structured output.

## Mutate Jobs

Complete one or more jobs:

```bash
graphile-worker complete 123 124
```

Permanently fail jobs:

```bash
graphile-worker fail 125 --reason "invalid payload"
```

If `--reason` is omitted, the CLI uses `Manually marked as failed`.

Reschedule jobs or update retry metadata:

```bash
graphile-worker reschedule 126 --run-at 2026-01-02T03:04:05Z
graphile-worker reschedule 127 --now
graphile-worker reschedule 128 --priority 7
graphile-worker reschedule 129 --attempts 1 --max-attempts 5
```

`reschedule` requires at least one of `--now`, `--run-at`, `--priority`,
`--attempts`, or `--max-attempts`.

Remove a job by job key:

```bash
graphile-worker remove user-123-welcome
```

## Overview Commands

Print queue-wide job counts:

```bash
graphile-worker stats
```

List queues and queue lock state:

```bash
graphile-worker queues
```

List worker ids that currently hold job or queue locks:

```bash
graphile-worker workers
```

These commands also support `--json`.

## Maintenance

Run cleanup tasks:

```bash
graphile-worker cleanup
```

When no task is passed, all cleanup tasks run. To run selected tasks:

```bash
graphile-worker cleanup delete-permanently-failed-jobs
graphile-worker cleanup gc-task-identifiers gc-job-queues
```

Force unlock jobs and queues locked by worker ids:

```bash
graphile-worker force-unlock graphile_worker_deadbeef
graphile-worker force-unlock worker-a worker-b
```

Recover stale workers and orphan locks:

```bash
graphile-worker sweep-stale-workers
graphile-worker sweep-stale-workers --sweep-threshold 5m --recovery-delay 30s
```

`--sweep-threshold` is the time since last heartbeat before a worker is deemed
inactive. `--recovery-delay` is the delay before recovered jobs are eligible to
run again. Durations may be plain seconds or use second, minute, or hour units,
for example `300`, `30s`, `5m`, or `2h`.

Use dry-run mode to see which stale workers would be recovered without
unlocking or rescheduling jobs:

```bash
graphile-worker sweep-stale-workers --dry-run
```

## Admin UI

Serve the embedded admin UI and JSON management API:

```bash
graphile-worker admin
```

By default it listens on `127.0.0.1:5678` and uses HTTP Basic auth with username
`admin`. If no password is provided, the CLI generates one and prints it at
startup.

Useful options:

```bash
graphile-worker admin \
  --listen 127.0.0.1:5678 \
  --auth basic \
  --username admin \
  --password "$GRAPHILE_WORKER_ADMIN_PASSWORD"
```

Authentication modes are `basic`, `bearer`, `header`, and `none`.

| Mode | Options and environment |
| --- | --- |
| `basic` | `--username`, `--password`, or `GRAPHILE_WORKER_ADMIN_PASSWORD` |
| `bearer` | `--bearer-token` or `GRAPHILE_WORKER_ADMIN_BEARER_TOKEN` |
| `header` | `--header-name`, `--header-token`, or `GRAPHILE_WORKER_ADMIN_HEADER_TOKEN` |
| `none` | Allowed only when `--listen` uses a loopback address |

For read-only operation, disable mutating admin actions:

```bash
graphile-worker admin --read-only
```

## Operational Caveats

- The CLI opens a PostgreSQL connection pool for every command, including the
  long-running `admin` server. Tune `--max-connections` for operational
  environments.
- `--json` is intended for automation. Human-readable tables are tab-separated
  and may be easier to inspect in a terminal.
- `complete`, `fail`, `reschedule`, `remove`, `cleanup`, `force-unlock`,
  `sweep-stale-workers`, and `migrate` change database state.
- `force-unlock` unlocks by worker id. Prefer `sweep-stale-workers --dry-run`
  first when recovering from stale workers, then run without `--dry-run` after
  checking the listed worker ids.
- `--auth none` for the admin UI is rejected unless the listen address is
  loopback.
