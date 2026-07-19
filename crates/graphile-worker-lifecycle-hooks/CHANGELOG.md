# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.5](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.3.4...graphile_worker_lifecycle_hooks-v0.3.5) - 2026-07-19

### Fixed

- use SPDX license = "MIT" instead of license-file ([#473](https://github.com/leo91000/graphile_worker_rs/pull/473))

## [0.3.4](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.3.3...graphile_worker_lifecycle_hooks-v0.3.4) - 2026-06-27

### Other

- updated the following local packages: graphile_worker_database, graphile_worker_crontab_types, graphile_worker_job, graphile_worker_job_spec

## [0.3.3](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.3.2...graphile_worker_lifecycle_hooks-v0.3.3) - 2026-06-09

### Added

- *(recovery)* add worker heartbeat recovery and shutdown job return

### Other

- clean worker architecture
- *(recovery)* macroize recovery hook event

## [0.3.2](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.3.1...graphile_worker_lifecycle_hooks-v0.3.2) - 2026-05-23

### Other

- updated the following local packages: graphile_worker_database

## [0.3.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.3.0...graphile_worker_lifecycle_hooks-v0.3.1) - 2026-05-12

### Other

- organize admin UI and workspace deps
- add CLI crate and prefix crate folders

## [0.3.0](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.2.5...graphile_worker_lifecycle_hooks-v0.3.0) - 2026-05-11

### Added

- support multiple postgres drivers
- add async runtime support

### Other

- increase project coverage
- improve perf PR coverage
- improve worker throughput

## [0.2.5](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.2.4...graphile_worker_lifecycle_hooks-v0.2.5) - 2026-02-25

### Fixed

- update repository links in all crate manifests

## [0.2.4](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.2.3...graphile_worker_lifecycle_hooks-v0.2.4) - 2026-02-15

### Other

- updated the following local packages: graphile_worker_crontab_types

## [0.2.3](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.2.2...graphile_worker_lifecycle_hooks-v0.2.3) - 2026-02-04

### Other

- update Cargo.toml dependencies

## [0.2.2](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.2.1...graphile_worker_lifecycle_hooks-v0.2.2) - 2025-12-31

### Other

- updated the following local packages: graphile_worker_crontab_types

## [0.2.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.2.0...graphile_worker_lifecycle_hooks-v0.2.1) - 2025-12-25

### Other

- replace manual builders with derive_builder macro

## [0.2.0](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.1.1...graphile_worker_lifecycle_hooks-v0.2.0) - 2025-12-24

### Added

- [**breaking**] replace LifecycleHooks trait with Bevy-style observer API
- add Debug derive to LocalQueue context structs
- add LocalQueue for batch job fetching with lifecycle hooks

### Other

- unify interceptor API with intercept() method
- use LocalQueueMode enum instead of String in LocalQueueSetModeContext
- use emit methods for all observer hooks with parallel execution

## [0.1.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_lifecycle_hooks-v0.1.0...graphile_worker_lifecycle_hooks-v0.1.1) - 2025-12-22

### Other

- update Cargo.toml dependencies

## [0.1.0](https://github.com/leo91000/graphile_worker_rs/releases/tag/graphile_worker_lifecycle_hooks-v0.1.0) - 2025-12-06

### Added

- add before_job_schedule hook for payload transformation
- add lifecycle hooks with native async fn syntax

### Other

- add unit tests to improve patch coverage
