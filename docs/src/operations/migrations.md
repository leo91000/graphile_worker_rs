# Migrations

Graphile Worker RS stores its database objects in a PostgreSQL schema and tracks
the installed schema revision in a `migrations` table inside that schema. The
`graphile_worker_migrations` crate owns this setup and exposes the migration
runner used to install or update the schema.

The public entry point is `graphile_worker_migrations::migrate(database, schema)`.
It accepts a database connection/executor value that can be converted into the
crate database type, plus the schema name to manage.

```rust,ignore
use graphile_worker_migrations::migrate;

migrate(&database, "graphile_worker").await?;
```

## What migrate does

On each run, the migration runner reads:

- the PostgreSQL `server_version_num`
- the latest row in `<schema>.migrations`
- the latest row in `<schema>.migrations` marked as a breaking migration

If `<schema>.migrations` does not exist, the runner installs the base schema
tracking objects first:

```sql
create schema if not exists graphile_worker;

create table if not exists graphile_worker.migrations (
    id int primary key,
    ts timestamptz default now() not null,
    breaking boolean not null default false
);
```

After that, it runs every packaged migration whose number is greater than the
latest recorded migration id. Each migration runs in its own database
transaction. When the SQL completes, the runner inserts a row into the
`migrations` table with the migration id and whether that migration is marked as
breaking, then commits the transaction.

Running `migrate` again after the schema is current is expected to be harmless:
the runner sees the latest recorded revision and skips already-applied
migrations.

## Packaged revisions

Migration SQL is bundled in the `graphile_worker_migrations` crate under
`src/sql`. At runtime the crate loads those files into the
`GRAPHILE_WORKER_MIGRATIONS` registry in revision order.

The current package contains revisions `1` through `20`. The revisions marked as
breaking in the registry are:

```text
1, 3, 11, 13, 14, 16
```

The runner supports taking over from an existing Graphile Worker schema as long
as the `migrations` table accurately records the already-applied revision. For
example, if revision `1` is already present, `migrate` starts at revision `2`.

## Startup guidance

Run migrations before starting workers that enqueue, poll, or execute jobs:

```rust,ignore
use graphile_worker_migrations::migrate;

async fn boot(database: &Database) -> Result<(), Box<dyn std::error::Error>> {
    migrate(database, "graphile_worker").await?;

    // Start worker processes only after the schema is installed and current.
    Ok(())
}
```

This is especially important around breaking revisions. The runner refuses to
continue if the database has a newer breaking migration than the currently
running package supports. For example, a database whose migrations table records
a future breaking revision cannot safely be used by an older worker binary.

If the database records a future non-breaking revision, the runner logs a
warning and continues. That warning means the database schema is newer than the
current package knows about, so all running Graphile Worker RS versions should be
checked for compatibility.

## PostgreSQL version check

The migration package requires PostgreSQL 12 or newer. It checks
`current_setting('server_version_num')` and returns an incompatible-version error
for versions below `120000`.

On a fresh schema install, the PostgreSQL version is checked before creating the
schema and migrations table. On existing schemas, the version reported while
reading migration state is checked before pending migrations are applied.

## Locked jobs during migration 11

Migration `11` has a specific guard for locked jobs. If PostgreSQL returns SQL
state `22012` while applying that migration, the runner returns
`MigrateError::LockedJobInMigration11`.

Before applying a package version that still needs to run migration `11`, stop
workers cleanly and make sure jobs and queues are not left locked. Then rerun
`migrate`.

## Custom schemas

The schema argument controls where the Graphile Worker objects are installed:

```rust,ignore
migrate(&database, "my_worker_schema").await?;
```

Packaged migration SQL uses the `:GRAPHILE_WORKER_SCHEMA` placeholder
internally. The migration executor replaces that placeholder with the escaped
schema name before executing each SQL statement.
