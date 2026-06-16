# Admin UI

Graphile Worker RS includes an embedded admin UI for inspecting queues, jobs,
and workers, plus a JSON API for the same operations. The implementation is
split across three crates:

- `graphile_worker_admin_api` defines shared request and response types, and
  SQLx-backed read queries when built with the `sqlx` feature.
- `graphile_worker_admin_ui` serves the native Axum application, the HTML page,
  embedded CSS, JavaScript, WASM, and JSON API routes.
- `graphile_worker_admin_ui_client` contains the WASM client that runs in the
  browser.

The CLI uses these crates to expose the `graphile-worker admin` command.

## Starting the Server

The admin command starts a native HTTP server and connects it to the same
Postgres schema and `WorkerUtils` used by the rest of the CLI.

```bash
graphile-worker admin
```

By default it listens on `127.0.0.1:5678`, uses HTTP Basic authentication with
username `admin`, and generates a password when none is provided.

Useful options include:

```bash
graphile-worker admin --listen 127.0.0.1:5678
graphile-worker admin --auth basic --username admin
graphile-worker admin --auth bearer
graphile-worker admin --auth header --header-name x-graphile-worker-admin-token
graphile-worker admin --read-only
```

The CLI also reads these environment variables for configured secrets:

```bash
GRAPHILE_WORKER_ADMIN_PASSWORD="<password>" graphile-worker admin --auth basic
GRAPHILE_WORKER_ADMIN_BEARER_TOKEN="<token>" graphile-worker admin --auth bearer
GRAPHILE_WORKER_ADMIN_HEADER_TOKEN="<token>" graphile-worker admin --auth header
```

When a Basic password, bearer token, or header token is generated, the CLI
prints it at startup. Configured secrets are not printed.

## Native Embedding

Applications can build the Axum server directly with
`graphile_worker_admin_ui::AdminServerConfig` and `graphile_worker_admin_ui::serve`.

```rust,ignore
use graphile_worker::{Schema, WorkerUtils};
use graphile_worker_admin_ui::{AdminAuthConfig, AdminServerConfig};
use sqlx::PgPool;

async fn serve_admin(pool: PgPool, utils: WorkerUtils) -> anyhow::Result<()> {
    let config = AdminServerConfig::builder(pool, utils)
        .schema(Schema::default())
        .schema_name("graphile_worker")
        .listen_addr("127.0.0.1:5678".parse()?)
        .auth(AdminAuthConfig::basic_with_random_password("admin"))
        .read_only(false)
        .build()?;

    graphile_worker_admin_ui::serve(config).await?;
    Ok(())
}
```

When embedding the admin UI directly, the config builder defaults to:

- schema `Schema::default()`
- schema name `graphile_worker`
- listen address `127.0.0.1:4000`
- generated Basic auth for username `admin`
- read-write mode

## Authentication

The server supports four auth modes:

| Mode | Behavior |
| --- | --- |
| `basic` | Requires HTTP Basic credentials. This is the default CLI mode. |
| `bearer` | Requires `Authorization: Bearer <token>`. |
| `header` | Requires a configured header name whose value matches the token. |
| `none` | Requires no auth, but is only allowed on loopback listen addresses. |

Both the CLI auth builder and the native config validation reject unauthenticated
servers bound to non-loopback addresses.

For Basic auth, the server also applies page authentication to the browser page
itself. API routes are authenticated in every mode except `none`.

## CSRF and Security Headers

The server generates a CSRF token when it builds application state. Every
mutating API request must include that token in the
`x-graphile-worker-admin-csrf` header. The browser client reads the header name
and token from the rendered page/session data and sends it on writes.

The server adds security headers to all responses:

- `Cache-Control: no-store` for pages and API responses
- `Cache-Control: public, max-age=3600` for `/assets/*` and `/favicon.ico`
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `Referrer-Policy: no-referrer`
- a content security policy limited to same-origin assets and API calls

## Routes

The native server exposes the browser page, embedded assets, and JSON API from
one Axum router.

| Route | Method | Purpose |
| --- | --- | --- |
| `/` | `GET` | Render the admin page. |
| `/assets/admin.css` | `GET` | Embedded stylesheet. |
| `/assets/admin.js` | `GET` | Embedded JavaScript entrypoint. |
| `/assets/admin_ui.js` | `GET` | WASM bindgen JavaScript. |
| `/assets/admin_ui_bg.wasm` | `GET` | WASM client module. |
| `/favicon.ico` | `GET` | Embedded SVG favicon. |
| `/api/session` | `GET` | Return schema, read-only state, CSRF header name, and public auth summary. |
| `/api/overview` | `GET` | Return job counts, queues, locked workers, and active workers. |
| `/api/jobs` | `GET` | List jobs. |
| `/api/jobs` | `POST` | Add a job. |
| `/api/jobs/{id}` | `GET` | Fetch one listed job. |
| `/api/jobs/action` | `POST` | Complete, fail, run now, or reschedule selected jobs. |
| `/api/jobs/remove-by-key` | `POST` | Remove a job by key. |
| `/api/maintenance` | `POST` | Run maintenance operations. |

## Job Visibility

`GET /api/jobs` accepts these query parameters:

- `state`: `all`, `ready`, `scheduled`, `locked`, or `failed`
- `identifier`: task identifier filter
- `queue`: queue name filter
- `search`: text search filter
- `limit`: result limit, capped by the native route at 500
- `offset`: result offset

The response contains `ListedJob` rows with job metadata such as id, task
identifier, queue name, payload, priority, run time, attempts, last error, key,
lock owner, revision, flags, and availability.

```bash
curl -u "admin:${GRAPHILE_WORKER_ADMIN_PASSWORD}" \
  "http://127.0.0.1:5678/api/jobs?state=failed&limit=50"
```

## Job Mutations

Mutating job routes are disabled when the server runs with `--read-only` or
`AdminServerConfig::read_only(true)`.

Adding a job uses the shared `AddJobRequest` contract:

```json
{
  "identifier": "send_email",
  "payload": { "user_id": 123 },
  "queue": "mailers",
  "max_attempts": 25,
  "key": "email:123",
  "job_key_mode": "replace",
  "priority": 0,
  "flags": ["transactional"]
}
```

Supported job key modes are `replace`, `preserve-run-at`, and
`unsafe-dedupe`. When `job_key_mode` is set, `key` is required.

`POST /api/jobs/action` accepts an action and job ids:

```json
{
  "action": "reschedule",
  "ids": [42, 43],
  "run_at": "2026-01-01T00:00:00Z",
  "priority": 10,
  "attempts": 0,
  "max_attempts": 25
}
```

Supported actions are:

- `complete`
- `fail`
- `run-now`
- `reschedule`

`fail` uses the provided `reason` when present and non-empty. Otherwise it uses
the admin UI default reason. `reschedule` requires at least one of `run_at`,
`priority`, `attempts`, or `max_attempts`.

Jobs can also be removed by key:

```json
{
  "key": "email:123"
}
```

## Maintenance

`POST /api/maintenance` exposes selected `WorkerUtils` maintenance operations.
It is also disabled in read-only mode.

Supported actions are:

- `migrate`
- `cleanup`
- `force-unlock`
- `sweep-stale-workers`

Cleanup can run all cleanup tasks by omitting `cleanup_tasks`, or a selected
subset:

```json
{
  "action": "cleanup",
  "cleanup_tasks": [
    "delete-permanently-failed-jobs",
    "gc-task-identifiers",
    "gc-job-queues"
  ]
}
```

Force unlock requires at least one worker id:

```json
{
  "action": "force-unlock",
  "worker_ids": ["worker-1"]
}
```

Stale worker sweeping accepts optional thresholds in seconds and supports dry
runs:

```json
{
  "action": "sweep-stale-workers",
  "dry_run": true,
  "sweep_threshold_secs": 300,
  "recovery_delay_secs": 60
}
```

## Assets and Client Behavior

The native crate embeds the admin stylesheet, JavaScript, WASM bindgen output,
WASM module, and SVG favicon into the server binary. The browser client uses
same-origin requests, sends `Accept: application/json`, includes credentials,
and adds the CSRF header on writes.

For bearer and header auth modes, the browser client sends the token on API
requests after it has been entered in the UI. For Basic auth, the browser uses
same-origin credentials from the HTTP auth challenge.
