# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.5](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_database-v0.1.4...graphile_worker_database-v0.1.5) - 2026-06-13

### Other

- update Cargo.toml dependencies

## [0.1.4](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_database-v0.1.3...graphile_worker_database-v0.1.4) - 2026-06-09

### Fixed

- accept raw schema inputs in query APIs

### Other

- clean up migration loading
- clean worker architecture
- match listener identifier escaping
- escape identifiers without database query
- migrate to sqlx 0.9

## [0.1.3](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_database-v0.1.2...graphile_worker_database-v0.1.3) - 2026-05-23

### Fixed

- support mutable tokio-postgres transactions
- accept transaction executors for add_job

### Other

- remove driver-specific executor probing

## [0.1.2](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_database-v0.1.1...graphile_worker_database-v0.1.2) - 2026-05-12

### Other

- organize admin UI and workspace deps

## [0.1.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_database-v0.1.0...graphile_worker_database-v0.1.1) - 2026-05-11

### Fixed

- reconnect tokio-postgres listener

### Other

- *(deps)* update all non-major dependencies ([#407](https://github.com/leo91000/graphile_worker_rs/pull/407))
