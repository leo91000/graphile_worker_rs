# graphile_worker_cli

Command-line job management utility for `graphile_worker`.

The installed binary is `graphile-worker`. It connects to PostgreSQL using
`--database-url` or `DATABASE_URL`, and uses the `graphile_worker` schema by
default.

```bash
graphile-worker --database-url postgres://postgres:postgres@localhost/postgres migrate
DATABASE_URL=postgres://postgres:postgres@localhost/postgres graphile-worker add send_email --payload '{"to":"user@example.com"}'
graphile-worker list --state ready
graphile-worker complete 123 124
graphile-worker fail 125 --reason "invalid payload"
graphile-worker reschedule 126 --run-at 2026-01-02T03:04:05Z
graphile-worker remove cli-job-key
graphile-worker cleanup
graphile-worker force-unlock graphile_worker_deadbeef
```

Available commands:

- `migrate`
- `add`
- `list`
- `show`
- `complete`
- `fail`
- `reschedule`
- `remove`
- `cleanup`
- `force-unlock`
- `stats`
- `queues`
- `workers`
