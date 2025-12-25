# Graphile Worker RS

Rust PostgreSQL-backed job queue - a port of Graphile Worker from Node.js.

## Before Committing

Always run the following commands before committing:

```bash
just lint
just test-docker
```

## Development Commands

| Command | Description |
|---------|-------------|
| `just lint` | Format + check + clippy |
| `just test-docker` | Run tests with Docker PostgreSQL |
| `just test` | Run tests (requires DATABASE_URL) |
| `just check` | cargo check --all --all-targets |
| `just check-clippy` | cargo clippy with warnings as errors |
| `just check-fmt` | Check formatting |
| `just fmt` | Format code |
| `just coverage-docker` | Run coverage with tarpaulin |
| `just coverage-diff` | Run coverage and show diff against main (requires `diff-cover`) |

## Environment Variables

- `DATABASE_URL` - Required for tests (e.g., `postgres://postgres:postgres@localhost:5432/postgres`)

## Project Structure

Workspace with 12 crates under `crates/`:
- `task_handler` - Core task handler trait
- `task_details` - Task ID/identifier mapping
- `job` / `job_spec` - Job types and specification builder
- `ctx` / `extensions` - Worker context and plugin system
- `lifecycle_hooks` - Job lifecycle hooks
- `crontab_runner` / `crontab_types` / `crontab_parser` - Cron scheduling
- `migrations` - Database migrations
- `shutdown_signal` - Shutdown signal handling

## Code Conventions

### Error Handling
- Use `thiserror` for error types with `#[derive(Error)]`
- Create type aliases: `pub type Result<T> = core::result::Result<T, MyError>;`

### Async
- Use tokio as async runtime
- Use `#[tokio::test]` for async tests

### Patterns
- Builder pattern for configuration (e.g., `WorkerOptions`)
- Use `#[derive(Getters)]` from `getset` crate for auto-generated getters
- Early returns (guard clauses) over nested conditionals
- Use `indoc::formatdoc!` for multi-line SQL strings

### Testing
- Integration tests in `tests/`
- Test helpers in `tests/helpers.rs`
- Tests create isolated databases with UUID-based names
