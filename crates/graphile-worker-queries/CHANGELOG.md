# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker_queries-v0.1.0...graphile_worker_queries-v0.1.1) - 2026-06-13

### Other

- updated the following local packages: graphile_worker_database, graphile_worker_job, graphile_worker_job_spec

## [0.1.0](https://github.com/leo91000/graphile_worker_rs/releases/tag/graphile_worker_queries-v0.1.0) - 2026-06-09

### Added

- support opentelemetry 0.32

### Fixed

- reject mixed opentelemetry feature versions
- accept raw schema inputs in query APIs

### Other

- label query add job spans
- move trace injection to query inserts
- clarify schema input test
- cover schema input compatibility
- split sql queries crate
