# Contributing to graphile_worker_rs

Thank you for your interest in contributing to graphile_worker_rs!

## Getting Started

1. Fork the repository
2. Clone your fork locally
3. Set up your development environment (see below)
4. Create a feature branch from `main`

## Development Setup

### Prerequisites

- Rust (stable toolchain)
- Docker (for running PostgreSQL in tests)
- `diff-cover` (optional, for coverage diff reports): `pip install diff-cover`

### Running Tests

```bash
just test-docker
```

This will spin up a PostgreSQL container and run all tests.

### Linting

Before submitting a PR, ensure your code passes all checks:

```bash
just lint
```

This runs `cargo fmt`, `cargo check`, and `cargo clippy`.

## Pull Request Process

1. Ensure all tests pass with `just test-docker`
2. Ensure linting passes with `just lint`
3. Update documentation if needed
4. Keep commits focused and atomic
5. Write clear commit messages

## Code Style

- Follow standard Rust conventions
- Use `cargo fmt` for formatting
- Prefer early returns (guard clauses) over nested conditionals
- Use `thiserror` for error types
- Use builder pattern for configuration structs

## Reporting Issues

- Use GitHub Issues for bug reports and feature requests
- For security vulnerabilities, see [SECURITY.md](SECURITY.md)

## Code of Conduct

This project follows the Contributor Covenant. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
