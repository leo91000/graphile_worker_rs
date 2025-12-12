# Changelog

## [0.9.2](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.9.1...graphile_worker-v0.9.2) - 2025-12-12

### Other

- *(deps)* update all non-major dependencies ([#333](https://github.com/leo91000/graphile_worker_rs/pull/333))

## [0.9.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.9.0...graphile_worker-v0.9.1) - 2025-12-12

### Fixed

- use explicit feature flags in tracing module

### Other

- Add OpenTelemetry tracing

## [0.9.0](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.8.7...graphile_worker-v0.9.0) - 2025-12-06

### Added

- add before_job_schedule hook for payload transformation
- add lifecycle hooks with native async fn syntax

### Other

- add unit tests to improve patch coverage
- add coverage for HookResult::Retry, after_job_run, on_job_permanently_fail
- *(deps)* update rust crate uuid to 1.19.0 ([#325](https://github.com/leo91000/graphile_worker_rs/pull/325))
- *(deps)* update all non-major dependencies ([#323](https://github.com/leo91000/graphile_worker_rs/pull/323))
- *(deps)* update actions/checkout action to v6
- *(deps)* update rust crate syn to 2.0.111 ([#322](https://github.com/leo91000/graphile_worker_rs/pull/322))
- *(deps)* update rust crate syn to 2.0.110 ([#320](https://github.com/leo91000/graphile_worker_rs/pull/320))
- *(deps)* update rust crate quote to 1.0.42 ([#319](https://github.com/leo91000/graphile_worker_rs/pull/319))
- *(deps)* update rust crate syn to 2.0.109 ([#318](https://github.com/leo91000/graphile_worker_rs/pull/318))
- *(deps)* update rust crate tokio-util to 0.7.17 ([#315](https://github.com/leo91000/graphile_worker_rs/pull/315))

## [0.8.7](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.8.6...graphile_worker-v0.8.7) - 2025-10-31

### Other

- Add derive Clone
- *(deps)* update rust crate syn to 2.0.108 ([#313](https://github.com/leo91000/graphile_worker_rs/pull/313))
- *(deps)* update rust crate indoc to 2.0.7 ([#312](https://github.com/leo91000/graphile_worker_rs/pull/312))
- *(deps)* update postgres docker tag to v18
- *(deps)* update rust crate syn to 2.0.107 ([#310](https://github.com/leo91000/graphile_worker_rs/pull/310))
- *(deps)* update rust crate cfg-if to 1.0.4 ([#309](https://github.com/leo91000/graphile_worker_rs/pull/309))
- *(deps)* update rust crate tokio to 1.48.0 ([#308](https://github.com/leo91000/graphile_worker_rs/pull/308))
- Fix license declaration
- *(deps)* update all non-major dependencies ([#304](https://github.com/leo91000/graphile_worker_rs/pull/304))
- *(deps)* update rust crate serde to 1.0.228 ([#303](https://github.com/leo91000/graphile_worker_rs/pull/303))
- *(deps)* update rust crate serde to 1.0.227 ([#302](https://github.com/leo91000/graphile_worker_rs/pull/302))
- *(deps)* update rust crate serde to 1.0.226 ([#301](https://github.com/leo91000/graphile_worker_rs/pull/301))
- *(deps)* update rust crate anyhow to 1.0.100 ([#300](https://github.com/leo91000/graphile_worker_rs/pull/300))

## [0.8.6](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.8.5...graphile_worker-v0.8.6) - 2025-09-19

### Added

- WorkerContext helpers and schema threading

### Other

- *(deps)* update rust crate serde to 1.0.225 ([#297](https://github.com/leo91000/graphile_worker_rs/pull/297))
- *(deps)* update all non-major dependencies ([#296](https://github.com/leo91000/graphile_worker_rs/pull/296))
- *(deps)* update all non-major dependencies ([#295](https://github.com/leo91000/graphile_worker_rs/pull/295))
- *(deps)* update rust crate chrono to 0.4.42 ([#294](https://github.com/leo91000/graphile_worker_rs/pull/294))
- *(deps)* update rust crate uuid to 1.18.1 ([#292](https://github.com/leo91000/graphile_worker_rs/pull/292))
- *(deps)* update rust crate tracing-subscriber to 0.3.20 ([#291](https://github.com/leo91000/graphile_worker_rs/pull/291))
- *(deps)* update actions/checkout action to v5
- *(deps)* update rust crate thiserror to 2.0.16 ([#290](https://github.com/leo91000/graphile_worker_rs/pull/290))
- *(deps)* update rust crate cfg-if to 1.0.3 ([#289](https://github.com/leo91000/graphile_worker_rs/pull/289))
- *(deps)* update rust crate serde_json to 1.0.143 ([#288](https://github.com/leo91000/graphile_worker_rs/pull/288))
- *(deps)* update rust crate thiserror to 2.0.15 ([#286](https://github.com/leo91000/graphile_worker_rs/pull/286))
- *(deps)* update rust crate syn to 2.0.106 ([#285](https://github.com/leo91000/graphile_worker_rs/pull/285))
- *(deps)* update rust crate syn to 2.0.105 ([#284](https://github.com/leo91000/graphile_worker_rs/pull/284))
- *(deps)* update all non-major dependencies ([#283](https://github.com/leo91000/graphile_worker_rs/pull/283))
- *(deps)* update rust crate uuid to 1.18.0 ([#282](https://github.com/leo91000/graphile_worker_rs/pull/282))
- *(deps)* update all non-major dependencies ([#281](https://github.com/leo91000/graphile_worker_rs/pull/281))
- *(deps)* update rust crate tokio to 1.47.0 ([#280](https://github.com/leo91000/graphile_worker_rs/pull/280))
- *(deps)* update rust crate rand to 0.9.2 ([#278](https://github.com/leo91000/graphile_worker_rs/pull/278))
- *(deps)* update rust crate serde_json to 1.0.141 ([#277](https://github.com/leo91000/graphile_worker_rs/pull/277))
- *(deps)* update rust crate tokio to 1.46.1 ([#276](https://github.com/leo91000/graphile_worker_rs/pull/276))
- *(deps)* update rust crate tokio to 1.46.0 ([#275](https://github.com/leo91000/graphile_worker_rs/pull/275))
- *(deps)* update rust crate syn to 2.0.104 ([#274](https://github.com/leo91000/graphile_worker_rs/pull/274))
- *(deps)* update rust crate getset to 0.1.6 ([#273](https://github.com/leo91000/graphile_worker_rs/pull/273))
- *(deps)* update rust crate syn to 2.0.103 ([#271](https://github.com/leo91000/graphile_worker_rs/pull/271))

## [0.8.5](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.8.4...graphile_worker-v0.8.5) - 2025-06-10

### Added

- make Worker sendable by adding Send + Sync bounds to WorkerFn

### Other

- apply cargo fmt formatting
- *(deps)* update all non-major dependencies ([#270](https://github.com/leo91000/graphile_worker_rs/pull/270))
- *(deps)* update rust crate num_cpus to 1.17.0 ([#267](https://github.com/leo91000/graphile_worker_rs/pull/267))
- *(deps)* update rust crate tokio to 1.45.1 ([#266](https://github.com/leo91000/graphile_worker_rs/pull/266))
- *(deps)* update rust crate uuid to 1.17.0 ([#265](https://github.com/leo91000/graphile_worker_rs/pull/265))
- *(deps)* update rust crate sqlx to 0.8.6 ([#264](https://github.com/leo91000/graphile_worker_rs/pull/264))
- *(deps)* update rust crate tokio to 1.45.0 ([#263](https://github.com/leo91000/graphile_worker_rs/pull/263))
- *(config)* migrate config renovate.json5
- *(deps)* update rust crate chrono to 0.4.41 ([#261](https://github.com/leo91000/graphile_worker_rs/pull/261))
- *(deps)* update rust crate syn to 2.0.101 ([#259](https://github.com/leo91000/graphile_worker_rs/pull/259))

## [0.8.4](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.8.3...graphile_worker-v0.8.4) - 2025-04-24

### Other

- Improve documentation
- *(deps)* update rust crate tokio-util to 0.7.15 ([#258](https://github.com/leo91000/graphile_worker_rs/pull/258))
- *(deps)* update rust crate serde_qs to 0.15.0 ([#256](https://github.com/leo91000/graphile_worker_rs/pull/256))

## [0.8.3](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.8.2...graphile_worker-v0.8.3) - 2025-04-21

### Other

- Improve documentation

## [0.8.2](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.8.1...graphile_worker-v0.8.2) - 2025-04-17

### Other

- Add more tests for the JobSpecBuilder
- *(deps)* update dependency ubuntu to v24
- *(deps)* update rust crate nom to v8.0.0

## [0.8.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.8.0...graphile_worker-v0.8.1) - 2025-04-17

### Other

- *(deps)* update rust crate rand to 0.9.1 ([#253](https://github.com/leo91000/graphile_worker_rs/pull/253))
- *(deps)* update all non-major dependencies ([#252](https://github.com/leo91000/graphile_worker_rs/pull/252))
- *(deps)* update rust crate tokio to 1.44.2 ([#251](https://github.com/leo91000/graphile_worker_rs/pull/251))
- *(deps)* update rust crate once_cell to 1.21.3 ([#250](https://github.com/leo91000/graphile_worker_rs/pull/250))
- *(deps)* update rust crate once_cell to 1.21.2 ([#249](https://github.com/leo91000/graphile_worker_rs/pull/249))
- *(deps)* update rust crate uuid to 1.16.0 ([#248](https://github.com/leo91000/graphile_worker_rs/pull/248))
- *(deps)* update all non-major dependencies ([#247](https://github.com/leo91000/graphile_worker_rs/pull/247))
- *(deps)* update rust crate quote to 1.0.40 ([#246](https://github.com/leo91000/graphile_worker_rs/pull/246))
- *(deps)* update rust crate once_cell to 1.21.0 ([#245](https://github.com/leo91000/graphile_worker_rs/pull/245))
- *(deps)* update all non-major dependencies ([#244](https://github.com/leo91000/graphile_worker_rs/pull/244))
- *(deps)* update rust crate tokio to 1.44.0 ([#243](https://github.com/leo91000/graphile_worker_rs/pull/243))
- *(deps)* update rust crate serde_qs to 0.14.0 ([#242](https://github.com/leo91000/graphile_worker_rs/pull/242))
- *(deps)* update rust crate indoc to 2.0.6 ([#241](https://github.com/leo91000/graphile_worker_rs/pull/241))
- *(deps)* update all non-major dependencies ([#240](https://github.com/leo91000/graphile_worker_rs/pull/240))
- *(deps)* update rust crate getset to 0.1.5 ([#239](https://github.com/leo91000/graphile_worker_rs/pull/239))
- *(deps)* update rust crate uuid to 1.15.1 ([#238](https://github.com/leo91000/graphile_worker_rs/pull/238))
- *(deps)* update all non-major dependencies ([#237](https://github.com/leo91000/graphile_worker_rs/pull/237))
- *(deps)* update rust crate uuid to 1.14.0 ([#236](https://github.com/leo91000/graphile_worker_rs/pull/236))
- *(deps)* update all non-major dependencies ([#235](https://github.com/leo91000/graphile_worker_rs/pull/235))
- *(deps)* update rust crate uuid to 1.13.2 ([#234](https://github.com/leo91000/graphile_worker_rs/pull/234))
- *(deps)* update rust crate once_cell to 1.20.3 ([#233](https://github.com/leo91000/graphile_worker_rs/pull/233))
- *(deps)* update rust crate uuid to 1.13.1 ([#232](https://github.com/leo91000/graphile_worker_rs/pull/232))
- *(deps)* update rust crate syn to 2.0.98 ([#230](https://github.com/leo91000/graphile_worker_rs/pull/230))
- *(deps)* update rust crate syn to 2.0.97 ([#229](https://github.com/leo91000/graphile_worker_rs/pull/229))
- *(deps)* update rust crate serde_json to 1.0.138 ([#228](https://github.com/leo91000/graphile_worker_rs/pull/228))
- *(deps)* update rust crate rand to 0.9.0 ([#227](https://github.com/leo91000/graphile_worker_rs/pull/227))
- *(deps)* update rust crate uuid to 1.12.1 ([#226](https://github.com/leo91000/graphile_worker_rs/pull/226))
- *(deps)* update rust crate getset to 0.1.4 ([#225](https://github.com/leo91000/graphile_worker_rs/pull/225))
- *(deps)* update rust crate serde_json to 1.0.137 ([#224](https://github.com/leo91000/graphile_worker_rs/pull/224))
- *(deps)* update rust crate serde_json to 1.0.136 ([#223](https://github.com/leo91000/graphile_worker_rs/pull/223))
- *(deps)* update all non-major dependencies ([#222](https://github.com/leo91000/graphile_worker_rs/pull/222))
- *(deps)* update all non-major dependencies ([#221](https://github.com/leo91000/graphile_worker_rs/pull/221))
- *(deps)* update all non-major dependencies ([#220](https://github.com/leo91000/graphile_worker_rs/pull/220))
- *(deps)* update rust crate syn to 2.0.95 ([#219](https://github.com/leo91000/graphile_worker_rs/pull/219))
- *(deps)* update rust crate sqlx to 0.8.3 ([#218](https://github.com/leo91000/graphile_worker_rs/pull/218))
- *(deps)* update rust crate syn to 2.0.94 ([#217](https://github.com/leo91000/graphile_worker_rs/pull/217))
- *(deps)* update rust crate syn to 2.0.93 ([#216](https://github.com/leo91000/graphile_worker_rs/pull/216))
- *(deps)* update rust crate serde to 1.0.217 ([#215](https://github.com/leo91000/graphile_worker_rs/pull/215))
- *(deps)* update rust crate syn to 2.0.92 ([#214](https://github.com/leo91000/graphile_worker_rs/pull/214))
- *(deps)* update rust crate quote to 1.0.38 ([#213](https://github.com/leo91000/graphile_worker_rs/pull/213))
- *(deps)* update rust crate anyhow to 1.0.95 ([#212](https://github.com/leo91000/graphile_worker_rs/pull/212))
- *(deps)* update rust crate syn to 2.0.91 ([#211](https://github.com/leo91000/graphile_worker_rs/pull/211))
- *(deps)* update all non-major dependencies ([#210](https://github.com/leo91000/graphile_worker_rs/pull/210))
- *(deps)* update rust crate thiserror to 2.0.8 ([#209](https://github.com/leo91000/graphile_worker_rs/pull/209))
- *(deps)* update all non-major dependencies ([#207](https://github.com/leo91000/graphile_worker_rs/pull/207))
- *(deps)* update codecov/codecov-action action to v5
- *(deps)* update rust crate thiserror to v2
- *(deps)* update rust crate tracing to 0.1.41 ([#205](https://github.com/leo91000/graphile_worker_rs/pull/205))
- *(deps)* update rust crate syn to 2.0.89 ([#204](https://github.com/leo91000/graphile_worker_rs/pull/204))
- *(deps)* update all non-major dependencies ([#202](https://github.com/leo91000/graphile_worker_rs/pull/202))
- *(deps)* update rust crate thiserror to 1.0.69 ([#201](https://github.com/leo91000/graphile_worker_rs/pull/201))
- *(deps)* update rust crate tokio to 1.41.1 ([#200](https://github.com/leo91000/graphile_worker_rs/pull/200))
- *(deps)* update rust crate anyhow to 1.0.93
- *(deps)* update rust crate thiserror to 1.0.68
- *(deps)* update rust crate thiserror to 1.0.67
- Improve readability of RunJobError
- *(deps)* update rust crate syn to 2.0.87
- *(deps)* update rust crate anyhow to 1.0.92
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde to 1.0.214
- *(deps)* update postgres docker tag to v17
- *(deps)* update rust crate syn to 2.0.85
- *(deps)* update all non-major dependencies
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate syn to 2.0.82
- *(deps)* update rust crate syn to 2.0.81
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde_json to 1.0.131
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde_json to 1.0.129
- *(deps)* update rust crate uuid to 1.11.0
- *(deps)* update rust crate derive_builder to 0.20.2
- *(deps)* update rust crate once_cell to 1.20.2
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate anyhow to 1.0.89
- *(deps)* update rust crate once_cell to 1.20.0
- *(deps)* update rust crate anyhow to 1.0.88
- *(deps)* update rust crate getset to 0.1.3
- *(deps)* update all non-major dependencies
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate sqlx to 0.8.2
- *(deps)* update rust crate syn to 2.0.77
- *(deps)* update rust crate tokio to 1.40.0
- *(deps)* update rust crate derive_builder to 0.20.1
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde_json to 1.0.127
- *(deps)* update rust crate quote to 1.0.37
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde to 1.0.208
- *(deps)* update rust crate serde_json to 1.0.125
- *(deps)* update rust crate serde to 1.0.207
- *(deps)* update all non-major dependencies
- Use assert instead of if + panic in case in concurrency equals to
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde to 1.0.205
- *(deps)* update rust crate serde_json to 1.0.122
- *(deps)* update rust crate serde_json to 1.0.121
- Fix new `refining_impl_trait` lint in example
- *(deps)* update rust crate tokio to 1.39.2
- *(deps)* update rust crate tokio to 1.39.1
- *(deps)* update rust crate sqlx to 0.8.0
- *(deps)* update rust crate syn to 2.0.72
- *(deps)* update rust crate thiserror to 1.0.63
- *(deps)* update rust crate tokio to 1.38.1
- *(deps)* update rust crate syn to 2.0.71
- *(deps)* update rust crate thiserror to 1.0.62
- *(deps)* update rust crate uuid to 1.10.0
- *(deps)* update rust crate syn to 2.0.70
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde_json to 1.0.119
- *(deps)* update rust crate serde_json to 1.0.118
- *(deps)* update rust crate uuid to 1.9.1
- *(deps)* update rust crate uuid to 1.9.0
- *(deps)* update rust crate syn to 2.0.68
- *(deps)* update rust crate syn to 2.0.67
- *(deps)* update rust crate tokio to 1.38.0

## [0.8.0](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.7.2...graphile_worker-v0.8.0) - 2024-05-29

### Added
- Introduce IntoTaskHandlerResult trait
- Job stream now yield as much job as the concurrency option defines
- Add get_ext() helper method on WorkerContext

### Other
- Add app state documentation in README

## [0.7.2](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.7.1...graphile_worker-v0.7.2) - 2024-05-28

### Added
- Add extensions supports

### Other
- *(deps)* update rust crate serde to 1.0.203
- *(deps)* update rust crate syn to 2.0.66
- *(deps)* update rust crate syn to 2.0.65
- *(deps)* update rust crate anyhow to 1.0.86
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate syn to 2.0.64
- *(deps)* update rust crate serde to 1.0.202
- *(deps)* update rust crate syn to 2.0.63
- *(deps)* update rust crate syn to 2.0.62

## [0.7.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.7.0...graphile_worker-v0.7.1) - 2024-05-08

### Other
- *(deps)* update all non-major dependencies
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate tokio-util to 0.7.11
- *(deps)* update rust crate serde to 1.0.200
- *(deps)* update rust crate serde to 1.0.199
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde to 1.0.198
- *(deps)* update rust crate serde_json to 1.0.116
- *(deps)* update rust crate chrono to 0.4.38
- *(deps)* update rust crate syn to 2.0.59
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde_qs to 0.13.0
- *(deps)* update rust crate syn to 2.0.58
- *(deps)* update rust crate syn to 2.0.57
- *(deps)* update rust crate tokio to 1.37.0
- *(deps)* update rust crate chrono to 0.4.37
- *(deps)* update rust crate serde_json to 1.0.115
- *(deps)* update rust crate syn to 2.0.55
- *(deps)* update rust crate indoc to 2.0.5
- *(deps)* update rust crate uuid to 1.8.0
- *(deps)* update rust crate syn to 2.0.53
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate chrono to 0.4.35
- *(deps)* update rust crate syn to 2.0.52

## [0.7.0](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.6.2...graphile_worker-v0.7.0) - 2024-02-28

### Added
- Try simplifying TaskHandler trait
- Add more metadata field to job crate
- Extract Job struct into its own crate
- Add JobSpec builder

### Other
- fix README invalid code
- Refactor from macro based to trait based
- *(deps)* update rust crate syn to 2.0.51
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate anyhow to 1.0.80
- Remove unecessary whitespace on queries
- *(deps)* update rust crate syn to 2.0.49
- Exclude macros crate from coverage since coverage can't be

## [0.6.2](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.6.1...graphile_worker-v0.6.2) - 2024-02-14

### Other
- Include README in rust docs
- Update README
- *(deps)* update rust crate thiserror to 1.0.57
- *(deps)* update codecov/codecov-action action to v4

## [0.6.1](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.6.0...graphile_worker-v0.6.1) - 2024-02-12

### Other
- Update badge
- Include badge in README
- remove unused file
- Add cron test
- Verbose tarpaulin
- Add codecov token
- Fix coverage path
- Add more time for runs_jobs_in_parallel to process job
- Rename tarpaulin job to coverage
- Split check into multiple files
- Wait more time for jobs to be processed for tarpaulin
- Add `.run()` test
- *(deps)* update rust crate chrono to 0.4.34

## [0.6.0](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.5.0...graphile_worker-v0.6.0) - 2024-02-07

### Added
- Added cleanup, completed_jobs, force_unlock_workers,
- Make add_job util returns the DbJob
- Add migration 11 locked job error
- Add final run_once tests
- Add remove_job helper

### Other
- [**breaking**] Rename helpers mod to worker_utils
- Make add_job return Job instead of DbJob
- More run_once tests

## [0.5.0](https://github.com/leo91000/graphile_worker_rs/compare/graphile_worker-v0.4.0...graphile_worker-v0.5.0) - 2024-02-04

### Other
- Add more complex test cases for run_once
- Add more run_once test
- Rename test file
- Add more run_once cases & refactor
- simplify job_count code
- Add more integration tests
- *(deps)* update rust crate tokio to 1.36.0
- Add the first integration test
- Rename create_helpers method to create_utils
- Fix ci permissions
- release

## [0.4.0](https://github.com/leo91000/graphile_worker_rs/releases/tag/graphile_worker-v0.4.0) - 2024-01-31

### Added
- Port breaking migration algorithm
- Add pg version checking
- Add initial support for breaking
- Add run_once
- Sync with latest graphile worker changes
- add_job & add_raw_job
- Add helpers
- Make simple macro example working
- Add task macro
- Add task handler definitions
- Abort running tasks 5 seconds after shutdown signal
- Handle job_key & job_key_mode in cron
- Add release xtask ([#32](https://github.com/leo91000/graphile_worker_rs/pull/32))
- Cron runner done ✔️
- Add backfill handling for crontab
- Added utilities for `CrontabFill`
- Handle process job error and stop stream
- Done with parsing crontab
- All parsing done
- Add query parser
- Can now complete jobs !
- First working POC of hashmap of async fns
- Add migration
- first commit

### Fixed
- Rustfmt
- Fix a bug where jobs would block event loop
- Add correct feature flag for sqlx dependency
- Fix typo in Cargo package keywords
- use modulus instead of substraction for duration remaining

### Other
- Remove xtask
- Add release workflow
- Rename lib to graphile_worker
- *(deps)* update rust crate itertools to 0.12.1
- Fix tests
- *(deps)* update rust crate serde_json to 1.0.113
- Add comments
- Update flake dependencies
- Update flake buildInputs
- Add nix flake
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate chrono to 0.4.33
- *(deps)* update rust crate chrono to 0.4.32
- *(deps)* update rust crate regex to 1.10.3
- *(deps)* update rust crate clap to 4.4.18
- *(deps)* update rust crate clap to 4.4.17
- *(deps)* update rust crate clap to 4.4.16
- *(deps)* update rust crate clap to 4.4.15
- *(deps)* update rust crate clap to 4.4.14
- *(deps)* update rust crate serde to 1.0.195
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde_json to 1.0.111
- *(deps)* update rust crate syn to 2.0.47
- Add mroe license
- Add more precision in README.md
- Add license & fix typos in README
- *(deps)* update all non-major dependencies
- Add comments
- *(deps)* update rust crate syn to 2.0.45
- *(release)* archimedes@0.4.0
- *(release)* archimedes_migrations@0.3.0
- *(release)* archimedes_macros@0.2.0
- *(release)* archimedes_crontab_runner@0.5.0
- *(release)* archimedes_crontab_parser@0.5.0
- *(release)* archimedes_task_handler@0.2.0
- *(release)* archimedes_shutdown_signal@0.3.0
- *(release)* archimedes_crontab_types@0.5.0
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate quote to 1.0.34
- Fmt
- Add code comment on WorkerHelpers and add_job
- Fix invalid example in README
- Reduce README.md example
- Remove invalid chunks in README.md
- Update README
- Add comment on the WorkerHelpers::new method
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate clap to 4.4.12
- *(deps)* update actions/checkout action to v4
- *(deps)* update rust crate anyhow to 1.0.77
- Remove unused import
- Remove duration in TaskAbort timeout log
- *(deps)* update rust crate thiserror to 1.0.52
- *(deps)* update all non-major dependencies
- Add comments to crontab_types
- Add more cases in should_run_at doc tests
- *(deps)* update rust crate clap to 4.2.5
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate tracing-subscriber to 0.3.17
- *(deps)* update rust crate regex to 1.8.1
- *(deps)* update rust crate regex to 1.8.0
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate clap to 4.2.3
- *(deps)* update rust crate clap to 4.2.2
- *(deps)* update rust crate serde_json to 1.0.96
- *(deps)* update rust crate serde to 1.0.160
- *(deps)* update rust crate futures to 0.3.28
- *(deps)* update rust crate clap to 4.2.1
- *(deps)* update rust crate clap to 4.2.0
- *(deps)* update rust crate serde to 1.0.159
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde_json to 1.0.95
- *(deps)* update rust crate regex to 1.7.3
- *(deps)* update rust crate clap to 4.1.13
- *(deps)* update rust crate clap to 4.1.12
- *(release)* archimedes@0.3.3
- *(release)* Fix changelog not using latest tag for new changelog
- *(release)* Remove unused import
- *(release)* archimedes@0.3.2
- *(release)* archimedes_migrations@0.2.4
- *(release)* archimedes_crontab_runner@0.4.2
- *(release)* archimedes_crontab_parser@0.4.2
- *(release)* archimedes_shutdown_signal@0.2.4
- *(release)* Add URL to tag release in github
- Print error to STERR instead of STDOUT
- Use `git push --tags` command instead of `git push --follow-tags`
- *(release)* archimedes@0.3.1
- *(release)* archimedes_migrations@0.2.3
- *(release)* archimedes_crontab_runner@0.4.1
- *(release)* archimedes_crontab_parser@0.4.1
- *(release)* archimedes_shutdown_signal@0.2.3
- Fix release script adding dependencies
- *(deps)* update rust crate toml_edit to 0.19.8
- *(deps)* update all non-major dependencies
- Release script now update dependencies version for non updated
- *(release)* archimedes@0.3.0
- *(release)* archimedes_migrations@0.2.2
- *(release)* archimedes_crontab_runner@0.4.0
- *(release)* archimedes_crontab_parser@0.4.0
- *(release)* archimedes_shutdown_signal@0.2.2
- *(release)* archimedes_crontab_types@0.4.0
- Update release script
- *(shutdown_signal)* Use tokio macros features
- *(release)* archimedes@0.1.0
- *(release)* archimedes_migrations@0.1.1
- *(release)* archimedes_crontab_runner@0.2.0
- *(release)* archimedes_crontab_parser@0.2.0
- *(release)* archimedes_shutdown_signal@0.1.1
- *(release)* archimedes_crontab_types@0.2.0
- *(deps)* update rust crate serde to 1.0.158
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate serde to 1.0.156
- *(deps)* update rust crate chrono to 0.4.24
- *(deps)* update rust crate serde to 1.0.155
- *(deps)* update rust crate futures to 0.3.27
- *(deps)* update rust crate serde to 1.0.154
- *(deps)* update rust crate serde to 1.0.153
- *(deps)* update rust crate thiserror to 1.0.39
- *(deps)* update rust crate serde_json to 1.0.94
- *(deps)* update rust crate serde_qs to 0.12.0
- *(deps)* update rust crate tokio to 1.26.0
- *(deps)* update rust crate once_cell to 1.17.1
- *(deps)* update rust crate serde_json to 1.0.93
- *(deps)* update all non-major dependencies
- *(deps)* update rust crate futures to 0.3.26
- *(deps)* update rust crate tokio to 1.25.0
- *(deps)* update rust crate tokio to 1.24.2
- *(deps)* update rust crate nom to 7.1.3
- *(deps)* update rust crate serde_qs to 0.11.0
- *(deps)* update rust crate tokio to 1.24.1
- *(deps)* update rust crate tokio to 1.24.0
- *(deps)* update rust crate tokio to 1.23.1
- *(deps)* update rust crate nom to 7.1.2
- *(deps)* update rust crate once_cell to 1.17.0
- *(deps)* update rust crate serde to 1.0.152
- Adds signal handling
- *(deps)* update rust crate num_cpus to 1.15.0
- Fix error in code block in README
- Add differences with graphile-worker in README
- Add crontab runner to the worker main run function
- Add license field for crontab_runner toml file
- Specify version for workspace packages
- Remove keywords and categories from packages metadata
- Include https:// in package metadata documentation and homepage
- Add license to crontab_types
- Prepare packages for publishing
- Apply clippy lint
- *(deps)* update all non-major dependencies
- Remove unused deps
- Add renovate
- Remove all features flags on clippy check
- Remove nightly flag
- Allow dead code for temporarly unused function
- Clippy fixes
- Add CI
- Remove unused test
- Improve README
- Fix typo
- Add README
- Refactor folder structure
- first attempts at crontab_runner
- Use u32 for crontab value
- Update schedule_crontab_jobs_at signature
- Added should_run_at documentation
- Add CrontabTimer tests
- Refactor folder structure
- reexport error kind
- Remove unused regexes module
- Replace manual digit parsing with character::complete::u8
- attempt at nom parsing crontab
- cleanup
- Remove empty file
- Refactor folder structure
- Remove unused Error
- Refactor worker
- Added example
- dynamic fn map
- Clippy fixes
- extract escape_identifier
- Remove .env file
- Gitignore .env file

## [0.3.3](https://github.com/leo91000/archimedes/releases/tag/archimedes@0.3.3)


### ✨Features

* feat: Sync with latest graphile worker changes ([ab25f29](https://github.com/leo91000/archimedes/commit/ab25f29))
* feat: add_job & add_raw_job ([97133ae](https://github.com/leo91000/archimedes/commit/97133ae))
* feat: Add helpers ([0af898b](https://github.com/leo91000/archimedes/commit/0af898b))
* feat: Make simple macro example working ([5ceb9b1](https://github.com/leo91000/archimedes/commit/5ceb9b1))
* feat: Add task macro ([e5106f7](https://github.com/leo91000/archimedes/commit/e5106f7))
* feat: Add task handler definitions ([3c38898](https://github.com/leo91000/archimedes/commit/3c38898))
* feat: Abort running tasks 5 seconds after shutdown signal ([adfb5a8](https://github.com/leo91000/archimedes/commit/adfb5a8))
* feat: Handle job_key & job_key_mode in cron ([faa9d12](https://github.com/leo91000/archimedes/commit/faa9d12))
* feat: Add release xtask (#32) ([f7fee4d](https://github.com/leo91000/archimedes/commit/f7fee4d))
* feat: Cron runner done ✔️ ([361906e](https://github.com/leo91000/archimedes/commit/361906e))
* feat: Add backfill handling for crontab ([3775f4f](https://github.com/leo91000/archimedes/commit/3775f4f))
* feat: Added utilities for `CrontabFill` ([59bb0cf](https://github.com/leo91000/archimedes/commit/59bb0cf))
* feat: Handle process job error and stop stream ([6301761](https://github.com/leo91000/archimedes/commit/6301761))
* feat: Done with parsing crontab ([39fba1a](https://github.com/leo91000/archimedes/commit/39fba1a))
* feat: All parsing done ([75c5429](https://github.com/leo91000/archimedes/commit/75c5429))
* feat: Add query parser ([579e34d](https://github.com/leo91000/archimedes/commit/579e34d))
* feat: Can now complete jobs ! ([efd829a](https://github.com/leo91000/archimedes/commit/efd829a))
* feat: First working POC of hashmap of async fns ([e2e30dc](https://github.com/leo91000/archimedes/commit/e2e30dc))
* feat: Add migration ([26492a1](https://github.com/leo91000/archimedes/commit/26492a1))
* feat: first commit ([0cd3b97](https://github.com/leo91000/archimedes/commit/0cd3b97))

### 🐛 Fixes

* fix: Rustfmt ([39841ad](https://github.com/leo91000/archimedes/commit/39841ad))
* fix: Fix a bug where jobs would block event loop ([4861dd4](https://github.com/leo91000/archimedes/commit/4861dd4))
* fix: Add correct feature flag for sqlx dependency ([15f8330](https://github.com/leo91000/archimedes/commit/15f8330))
* fix: Fix typo in Cargo package keywords ([012d4ee](https://github.com/leo91000/archimedes/commit/012d4ee))
* fix: use modulus instead of substraction for duration remaining ([9ea0a52](https://github.com/leo91000/archimedes/commit/9ea0a52))

### 🧹 chores

* chore(release): archimedes_migrations@0.3.0 ([74223cd](https://github.com/leo91000/archimedes/commit/74223cd))
* chore(release): archimedes_macros@0.2.0 ([f908de2](https://github.com/leo91000/archimedes/commit/f908de2))
* chore(release): archimedes_crontab_runner@0.5.0 ([3c8c54a](https://github.com/leo91000/archimedes/commit/3c8c54a))
* chore(release): archimedes_crontab_parser@0.5.0 ([76b56e8](https://github.com/leo91000/archimedes/commit/76b56e8))
* chore(release): archimedes_task_handler@0.2.0 ([8e62139](https://github.com/leo91000/archimedes/commit/8e62139))
* chore(release): archimedes_shutdown_signal@0.3.0 ([8a213aa](https://github.com/leo91000/archimedes/commit/8a213aa))
* chore(release): archimedes_crontab_types@0.5.0 ([29146b9](https://github.com/leo91000/archimedes/commit/29146b9))
* chore(deps): update all non-major dependencies ([0411732](https://github.com/leo91000/archimedes/commit/0411732))
* chore(deps): update rust crate quote to 1.0.34 ([5866514](https://github.com/leo91000/archimedes/commit/5866514))
* chore: Fmt ([170ef3c](https://github.com/leo91000/archimedes/commit/170ef3c))
* chore: Fix invalid example in README ([9ec5689](https://github.com/leo91000/archimedes/commit/9ec5689))
* chore: Reduce README.md example ([2098c1c](https://github.com/leo91000/archimedes/commit/2098c1c))
* chore: Remove invalid chunks in README.md ([0d31457](https://github.com/leo91000/archimedes/commit/0d31457))
* chore: Update README ([ca53165](https://github.com/leo91000/archimedes/commit/ca53165))
* chore: Add comment on the WorkerHelpers::new method ([422a1b2](https://github.com/leo91000/archimedes/commit/422a1b2))
* chore(deps): update all non-major dependencies ([b1a96b6](https://github.com/leo91000/archimedes/commit/b1a96b6))
* chore(deps): update rust crate clap to 4.4.12 ([1ecbcb1](https://github.com/leo91000/archimedes/commit/1ecbcb1))
* chore(deps): update actions/checkout action to v4 ([588dd60](https://github.com/leo91000/archimedes/commit/588dd60))
* chore(deps): update rust crate anyhow to 1.0.77 ([163c92a](https://github.com/leo91000/archimedes/commit/163c92a))
* chore: Remove unused import ([31706a8](https://github.com/leo91000/archimedes/commit/31706a8))
* chore: Remove duration in TaskAbort timeout log ([5afcbdd](https://github.com/leo91000/archimedes/commit/5afcbdd))
* chore(deps): update rust crate thiserror to 1.0.52 ([139f870](https://github.com/leo91000/archimedes/commit/139f870))
* chore(deps): update all non-major dependencies ([554a4c0](https://github.com/leo91000/archimedes/commit/554a4c0))
* chore: Add comments to crontab_types ([8aebeed](https://github.com/leo91000/archimedes/commit/8aebeed))
* chore: Add more cases in should_run_at doc tests ([a3465cc](https://github.com/leo91000/archimedes/commit/a3465cc))
* chore(deps): update rust crate clap to 4.2.5 ([985cc77](https://github.com/leo91000/archimedes/commit/985cc77))
* chore(deps): update all non-major dependencies ([3731c13](https://github.com/leo91000/archimedes/commit/3731c13))
* chore(deps): update rust crate tracing-subscriber to 0.3.17 ([df8531c](https://github.com/leo91000/archimedes/commit/df8531c))
* chore(deps): update rust crate regex to 1.8.1 ([800dbd8](https://github.com/leo91000/archimedes/commit/800dbd8))
* chore(deps): update rust crate regex to 1.8.0 ([fc8eb93](https://github.com/leo91000/archimedes/commit/fc8eb93))
* chore(deps): update all non-major dependencies ([e27c5f0](https://github.com/leo91000/archimedes/commit/e27c5f0))
* chore(deps): update rust crate clap to 4.2.3 ([9ec1634](https://github.com/leo91000/archimedes/commit/9ec1634))
* chore(deps): update rust crate clap to 4.2.2 ([1842e2f](https://github.com/leo91000/archimedes/commit/1842e2f))
* chore(deps): update rust crate serde_json to 1.0.96 ([d5ad5ce](https://github.com/leo91000/archimedes/commit/d5ad5ce))
* chore(deps): update rust crate serde to 1.0.160 ([1b17a9e](https://github.com/leo91000/archimedes/commit/1b17a9e))
* chore(deps): update rust crate futures to 0.3.28 ([9a59868](https://github.com/leo91000/archimedes/commit/9a59868))
* chore(deps): update rust crate clap to 4.2.1 ([e6faa49](https://github.com/leo91000/archimedes/commit/e6faa49))
* chore(deps): update rust crate clap to 4.2.0 ([6b66566](https://github.com/leo91000/archimedes/commit/6b66566))
* chore(deps): update rust crate serde to 1.0.159 ([4691d72](https://github.com/leo91000/archimedes/commit/4691d72))
* chore(deps): update all non-major dependencies ([60d004e](https://github.com/leo91000/archimedes/commit/60d004e))
* chore(deps): update rust crate serde_json to 1.0.95 ([10772ac](https://github.com/leo91000/archimedes/commit/10772ac))
* chore(deps): update rust crate regex to 1.7.3 ([5a9dc65](https://github.com/leo91000/archimedes/commit/5a9dc65))
* chore(deps): update rust crate clap to 4.1.13 ([515e6d6](https://github.com/leo91000/archimedes/commit/515e6d6))
* chore(deps): update rust crate clap to 4.1.12 ([41d544a](https://github.com/leo91000/archimedes/commit/41d544a))
* chore(release): archimedes@0.3.3 ([cf407df](https://github.com/leo91000/archimedes/commit/cf407df))
* chore(release): archimedes@0.3.2 ([bb5ab58](https://github.com/leo91000/archimedes/commit/bb5ab58))
* chore(release): archimedes_migrations@0.2.4 ([ad5a70d](https://github.com/leo91000/archimedes/commit/ad5a70d))
* chore(release): archimedes_crontab_runner@0.4.2 ([06bca65](https://github.com/leo91000/archimedes/commit/06bca65))
* chore(release): archimedes_crontab_parser@0.4.2 ([eff739d](https://github.com/leo91000/archimedes/commit/eff739d))
* chore(release): archimedes_shutdown_signal@0.2.4 ([0ca921b](https://github.com/leo91000/archimedes/commit/0ca921b))
* chore(release): archimedes@0.3.1 ([4316b3d](https://github.com/leo91000/archimedes/commit/4316b3d))
* chore(release): archimedes_migrations@0.2.3 ([4ea2dc3](https://github.com/leo91000/archimedes/commit/4ea2dc3))
* chore(release): archimedes_crontab_runner@0.4.1 ([3a5f858](https://github.com/leo91000/archimedes/commit/3a5f858))
* chore(release): archimedes_crontab_parser@0.4.1 ([c5cec18](https://github.com/leo91000/archimedes/commit/c5cec18))
* chore(release): archimedes_shutdown_signal@0.2.3 ([08fc8d7](https://github.com/leo91000/archimedes/commit/08fc8d7))
* chore(deps): update rust crate toml_edit to 0.19.8 ([828339d](https://github.com/leo91000/archimedes/commit/828339d))
* chore(deps): update all non-major dependencies ([85e3b4b](https://github.com/leo91000/archimedes/commit/85e3b4b))
* chore: Release script now update dependencies version for non updated packages ([25c97a0](https://github.com/leo91000/archimedes/commit/25c97a0))
* chore(release): archimedes@0.3.0 ([5911eee](https://github.com/leo91000/archimedes/commit/5911eee))
* chore(release): archimedes_migrations@0.2.2 ([d3ec037](https://github.com/leo91000/archimedes/commit/d3ec037))
* chore(release): archimedes_crontab_runner@0.4.0 ([6ec53f6](https://github.com/leo91000/archimedes/commit/6ec53f6))
* chore(release): archimedes_crontab_parser@0.4.0 ([be084df](https://github.com/leo91000/archimedes/commit/be084df))
* chore(release): archimedes_shutdown_signal@0.2.2 ([d894609](https://github.com/leo91000/archimedes/commit/d894609))
* chore(release): archimedes_crontab_types@0.4.0 ([e82bf12](https://github.com/leo91000/archimedes/commit/e82bf12))
* chore(shutdown_signal): Use tokio macros features ([674f1ec](https://github.com/leo91000/archimedes/commit/674f1ec))
* chore(release): archimedes@0.1.0 ([4b2b809](https://github.com/leo91000/archimedes/commit/4b2b809))
* chore(release): archimedes_migrations@0.1.1 ([c10a4d2](https://github.com/leo91000/archimedes/commit/c10a4d2))
* chore(release): archimedes_crontab_runner@0.2.0 ([7d03c47](https://github.com/leo91000/archimedes/commit/7d03c47))
* chore(release): archimedes_crontab_parser@0.2.0 ([06f7db2](https://github.com/leo91000/archimedes/commit/06f7db2))
* chore(release): archimedes_shutdown_signal@0.1.1 ([a1c332a](https://github.com/leo91000/archimedes/commit/a1c332a))
* chore(release): archimedes_crontab_types@0.2.0 ([a910b4f](https://github.com/leo91000/archimedes/commit/a910b4f))
* chore(deps): update rust crate serde to 1.0.158 ([4643dcb](https://github.com/leo91000/archimedes/commit/4643dcb))
* chore(deps): update all non-major dependencies ([629a3dd](https://github.com/leo91000/archimedes/commit/629a3dd))
* chore(deps): update rust crate serde to 1.0.156 ([86685ec](https://github.com/leo91000/archimedes/commit/86685ec))
* chore(deps): update rust crate chrono to 0.4.24 ([5676a7c](https://github.com/leo91000/archimedes/commit/5676a7c))
* chore(deps): update rust crate serde to 1.0.155 ([f98fde2](https://github.com/leo91000/archimedes/commit/f98fde2))
* chore(deps): update rust crate futures to 0.3.27 ([2163377](https://github.com/leo91000/archimedes/commit/2163377))
* chore(deps): update rust crate serde to 1.0.154 ([abddcf5](https://github.com/leo91000/archimedes/commit/abddcf5))
* chore(deps): update rust crate serde to 1.0.153 ([374f207](https://github.com/leo91000/archimedes/commit/374f207))
* chore(deps): update rust crate thiserror to 1.0.39 ([0790a3f](https://github.com/leo91000/archimedes/commit/0790a3f))
* chore(deps): update rust crate serde_json to 1.0.94 ([a969360](https://github.com/leo91000/archimedes/commit/a969360))
* chore(deps): update rust crate serde_qs to 0.12.0 ([94df8c9](https://github.com/leo91000/archimedes/commit/94df8c9))
* chore(deps): update rust crate tokio to 1.26.0 ([9d8cd06](https://github.com/leo91000/archimedes/commit/9d8cd06))
* chore(deps): update rust crate once_cell to 1.17.1 ([9e9d8aa](https://github.com/leo91000/archimedes/commit/9e9d8aa))
* chore(deps): update rust crate serde_json to 1.0.93 ([7eb2054](https://github.com/leo91000/archimedes/commit/7eb2054))
* chore(deps): update all non-major dependencies ([3ecb31d](https://github.com/leo91000/archimedes/commit/3ecb31d))
* chore(deps): update rust crate futures to 0.3.26 ([8a89947](https://github.com/leo91000/archimedes/commit/8a89947))
* chore(deps): update rust crate tokio to 1.25.0 ([b28f180](https://github.com/leo91000/archimedes/commit/b28f180))
* chore(deps): update rust crate tokio to 1.24.2 ([035b7fa](https://github.com/leo91000/archimedes/commit/035b7fa))
* chore(deps): update rust crate nom to 7.1.3 ([5833efd](https://github.com/leo91000/archimedes/commit/5833efd))
* chore(deps): update rust crate serde_qs to 0.11.0 ([e6700bc](https://github.com/leo91000/archimedes/commit/e6700bc))
* chore(deps): update rust crate tokio to 1.24.1 ([9bc0335](https://github.com/leo91000/archimedes/commit/9bc0335))
* chore(deps): update rust crate tokio to 1.24.0 ([43ebddf](https://github.com/leo91000/archimedes/commit/43ebddf))
* chore(deps): update rust crate tokio to 1.23.1 ([d20c0b6](https://github.com/leo91000/archimedes/commit/d20c0b6))
* chore(deps): update rust crate nom to 7.1.2 ([038f4d2](https://github.com/leo91000/archimedes/commit/038f4d2))
* chore(deps): update rust crate once_cell to 1.17.0 ([c52bb61](https://github.com/leo91000/archimedes/commit/c52bb61))
* chore(deps): update rust crate serde to 1.0.152 ([a6115cb](https://github.com/leo91000/archimedes/commit/a6115cb))
* chore(deps): update rust crate num_cpus to 1.15.0 ([2a7ef10](https://github.com/leo91000/archimedes/commit/2a7ef10))
* chore: Fix error in code block in README ([3bf9f51](https://github.com/leo91000/archimedes/commit/3bf9f51))
* chore: Add differences with graphile-worker in README ([bce3367](https://github.com/leo91000/archimedes/commit/bce3367))
* chore: Add crontab runner to the worker main run function ([340445a](https://github.com/leo91000/archimedes/commit/340445a))
* chore: Add license field for crontab_runner toml file ([9266308](https://github.com/leo91000/archimedes/commit/9266308))
* chore: Specify version for workspace packages ([8e03f22](https://github.com/leo91000/archimedes/commit/8e03f22))
* chore: Remove keywords and categories from packages metadata ([798bded](https://github.com/leo91000/archimedes/commit/798bded))
* chore: Include https:// in package metadata documentation and homepage ([ebffd12](https://github.com/leo91000/archimedes/commit/ebffd12))
* chore: Add license to crontab_types ([3cd31cb](https://github.com/leo91000/archimedes/commit/3cd31cb))
* chore: Prepare packages for publishing ([5d99f5c](https://github.com/leo91000/archimedes/commit/5d99f5c))
* chore: Apply clippy lint ([66e3894](https://github.com/leo91000/archimedes/commit/66e3894))
* chore(deps): update all non-major dependencies ([bdb33af](https://github.com/leo91000/archimedes/commit/bdb33af))
* chore: Remove unused deps ([bb09685](https://github.com/leo91000/archimedes/commit/bb09685))
* chore: Remove nightly flag ([05b4c63](https://github.com/leo91000/archimedes/commit/05b4c63))
* chore: Allow dead code for temporarly unused function ([e5b4329](https://github.com/leo91000/archimedes/commit/e5b4329))
* chore: Clippy fixes ([506f98c](https://github.com/leo91000/archimedes/commit/506f98c))
* chore: Remove unused test ([5ac1deb](https://github.com/leo91000/archimedes/commit/5ac1deb))
* chore: Improve README ([d0434ce](https://github.com/leo91000/archimedes/commit/d0434ce))
* chore: Fix typo ([6557171](https://github.com/leo91000/archimedes/commit/6557171))
* chore: Add README ([5ab6f7f](https://github.com/leo91000/archimedes/commit/5ab6f7f))
* chore: Refactor folder structure ([ed29cec](https://github.com/leo91000/archimedes/commit/ed29cec))
* chore: Use u32 for crontab value ([956937e](https://github.com/leo91000/archimedes/commit/956937e))
* chore: Update schedule_crontab_jobs_at signature ([8d5e1e3](https://github.com/leo91000/archimedes/commit/8d5e1e3))
* chore: Refactor folder structure ([df41490](https://github.com/leo91000/archimedes/commit/df41490))
* chore: reexport error kind ([f6921a8](https://github.com/leo91000/archimedes/commit/f6921a8))
* chore: Remove unused regexes module ([c9cdf04](https://github.com/leo91000/archimedes/commit/c9cdf04))
* chore: Replace manual digit parsing with character::complete::u8 ([f5d680d](https://github.com/leo91000/archimedes/commit/f5d680d))
* chore: cleanup ([f7647a6](https://github.com/leo91000/archimedes/commit/f7647a6))
* chore: Remove empty file ([e748520](https://github.com/leo91000/archimedes/commit/e748520))
* chore: Refactor folder structure ([fdb0fc8](https://github.com/leo91000/archimedes/commit/fdb0fc8))
* chore: Remove unused Error ([ea794e6](https://github.com/leo91000/archimedes/commit/ea794e6))
* chore: Refactor worker ([2e06b42](https://github.com/leo91000/archimedes/commit/2e06b42))
* chore: Added example ([4273672](https://github.com/leo91000/archimedes/commit/4273672))
* chore: Clippy fixes ([db5ec81](https://github.com/leo91000/archimedes/commit/db5ec81))
* chore: extract escape_identifier ([cb423aa](https://github.com/leo91000/archimedes/commit/cb423aa))
* chore: Remove .env file ([edfc81c](https://github.com/leo91000/archimedes/commit/edfc81c))
* chore: Gitignore .env file ([4a61b9a](https://github.com/leo91000/archimedes/commit/4a61b9a))

### 🧪 Tests

* test: Add CrontabTimer tests ([a432ad8](https://github.com/leo91000/archimedes/commit/a432ad8))

### 📝 Docs

* docs: Add code comment on WorkerHelpers and add_job ([ddb97a8](https://github.com/leo91000/archimedes/commit/ddb97a8))
* docs: Added should_run_at documentation ([3b9fe96](https://github.com/leo91000/archimedes/commit/3b9fe96))

### 🤖 CI

* ci: Update release script ([3614a76](https://github.com/leo91000/archimedes/commit/3614a76))
* ci: Add renovate ([a78ffcf](https://github.com/leo91000/archimedes/commit/a78ffcf))
* ci: Remove all features flags on clippy check ([fb20e9b](https://github.com/leo91000/archimedes/commit/fb20e9b))
* ci: Add CI ([80d7fb5](https://github.com/leo91000/archimedes/commit/80d7fb5))

### 🛠 Dev

* dev(release): Fix changelog not using latest tag for new changelog ([eae80d6](https://github.com/leo91000/archimedes/commit/eae80d6))
* dev(release): Remove unused import ([04cd052](https://github.com/leo91000/archimedes/commit/04cd052))
* dev(release): Add URL to tag release in github ([6266cc2](https://github.com/leo91000/archimedes/commit/6266cc2))
* dev: Print error to STERR instead of STDOUT ([8fe3ecd](https://github.com/leo91000/archimedes/commit/8fe3ecd))
* dev: Use `git push --tags` command instead of `git push --follow-tags` ([625fd10](https://github.com/leo91000/archimedes/commit/625fd10))
* dev: Fix release script adding dependencies ([5c7cb61](https://github.com/leo91000/archimedes/commit/5c7cb61))

### 🚧 WIP

* wip: Adds signal handling ([a8d11b2](https://github.com/leo91000/archimedes/commit/a8d11b2))
* wip: first attempts at crontab_runner ([4c59b2e](https://github.com/leo91000/archimedes/commit/4c59b2e))
* wip: attempt at nom parsing crontab ([c48e972](https://github.com/leo91000/archimedes/commit/c48e972))
* wip: dynamic fn map ([8464613](https://github.com/leo91000/archimedes/commit/8464613))


## [0.3.2](https://github.com/leo91000/archimedes/releases/tag/archimedes@0.3.2)


### 🛠 Dev

* dev(release): Fix changelog not using latest tag for new changelog ([eae80d6](https://github.com/leo91000/archimedes/commit/eae80d6))
* dev(release): Remove unused import ([04cd052](https://github.com/leo91000/archimedes/commit/04cd052))


## [0.3.1](https://github.com/leo91000/archimedes/releases/tag/archimedes@0.3.1)


### 🧹 chores

* chore(release): archimedes_migrations@0.2.4 ([ad5a70d](https://github.com/leo91000/archimedes/commit/ad5a70d))
* chore(release): archimedes_crontab_runner@0.4.2 ([06bca65](https://github.com/leo91000/archimedes/commit/06bca65))
* chore(release): archimedes_crontab_parser@0.4.2 ([eff739d](https://github.com/leo91000/archimedes/commit/eff739d))
* chore(release): archimedes_shutdown_signal@0.2.4 ([0ca921b](https://github.com/leo91000/archimedes/commit/0ca921b))
* chore(release): archimedes@0.3.1 ([4316b3d](https://github.com/leo91000/archimedes/commit/4316b3d))
* chore(release): archimedes_migrations@0.2.3 ([4ea2dc3](https://github.com/leo91000/archimedes/commit/4ea2dc3))
* chore(release): archimedes_crontab_runner@0.4.1 ([3a5f858](https://github.com/leo91000/archimedes/commit/3a5f858))
* chore(release): archimedes_crontab_parser@0.4.1 ([c5cec18](https://github.com/leo91000/archimedes/commit/c5cec18))
* chore(release): archimedes_shutdown_signal@0.2.3 ([08fc8d7](https://github.com/leo91000/archimedes/commit/08fc8d7))
* chore(deps): update rust crate toml_edit to 0.19.8 ([828339d](https://github.com/leo91000/archimedes/commit/828339d))
* chore(deps): update all non-major dependencies ([85e3b4b](https://github.com/leo91000/archimedes/commit/85e3b4b))
* chore: Release script now update dependencies version for non updated packages ([25c97a0](https://github.com/leo91000/archimedes/commit/25c97a0))

### 🛠 Dev

* dev(release): Add URL to tag release in github ([6266cc2](https://github.com/leo91000/archimedes/commit/6266cc2))
* dev: Print error to STERR instead of STDOUT ([8fe3ecd](https://github.com/leo91000/archimedes/commit/8fe3ecd))
* dev: Use `git push --tags` command instead of `git push --follow-tags` ([625fd10](https://github.com/leo91000/archimedes/commit/625fd10))
* dev: Fix release script adding dependencies ([5c7cb61](https://github.com/leo91000/archimedes/commit/5c7cb61))


## 0.3.0


### 🧹 chores

* chore(release): archimedes_migrations@0.2.3 ([4ea2dc3](https://github.com/leo91000/archimedes/commit/4ea2dc3))
* chore(release): archimedes_crontab_runner@0.4.1 ([3a5f858](https://github.com/leo91000/archimedes/commit/3a5f858))
* chore(release): archimedes_crontab_parser@0.4.1 ([c5cec18](https://github.com/leo91000/archimedes/commit/c5cec18))
* chore(release): archimedes_shutdown_signal@0.2.3 ([08fc8d7](https://github.com/leo91000/archimedes/commit/08fc8d7))
* chore(deps): update rust crate toml_edit to 0.19.8 ([828339d](https://github.com/leo91000/archimedes/commit/828339d))
* chore(deps): update all non-major dependencies ([85e3b4b](https://github.com/leo91000/archimedes/commit/85e3b4b))
* chore: Release script now update dependencies version for non updated packages ([25c97a0](https://github.com/leo91000/archimedes/commit/25c97a0))

### 🛠 Dev

* dev: Fix release script adding dependencies ([5c7cb61](https://github.com/leo91000/archimedes/commit/5c7cb61))

## 0.2.0


### ✨Features

* feat: Add release xtask (#32) ([f7fee4d](https://github.com/leo91000/archimedes/commit/f7fee4d))
* feat: Cron runner done ✔️ ([361906e](https://github.com/leo91000/archimedes/commit/361906e))
* feat: Add backfill handling for crontab ([3775f4f](https://github.com/leo91000/archimedes/commit/3775f4f))
* feat: Added utilities for `CrontabFill` ([59bb0cf](https://github.com/leo91000/archimedes/commit/59bb0cf))
* feat: Handle process job error and stop stream ([6301761](https://github.com/leo91000/archimedes/commit/6301761))
* feat: Done with parsing crontab ([39fba1a](https://github.com/leo91000/archimedes/commit/39fba1a))
* feat: All parsing done ([75c5429](https://github.com/leo91000/archimedes/commit/75c5429))
* feat: Add query parser ([579e34d](https://github.com/leo91000/archimedes/commit/579e34d))
* feat: Can now complete jobs ! ([efd829a](https://github.com/leo91000/archimedes/commit/efd829a))
* feat: First working POC of hashmap of async fns ([e2e30dc](https://github.com/leo91000/archimedes/commit/e2e30dc))
* feat: Add migration ([26492a1](https://github.com/leo91000/archimedes/commit/26492a1))
* feat: first commit ([0cd3b97](https://github.com/leo91000/archimedes/commit/0cd3b97))

### 🐛 Fixes

* fix: Add correct feature flag for sqlx dependency ([15f8330](https://github.com/leo91000/archimedes/commit/15f8330))
* fix: Fix typo in Cargo package keywords ([012d4ee](https://github.com/leo91000/archimedes/commit/012d4ee))
* fix: use modulus instead of substraction for duration remaining ([9ea0a52](https://github.com/leo91000/archimedes/commit/9ea0a52))

### 🧹 chores

* chore(release): archimedes_migrations@0.2.2 ([d3ec037](https://github.com/leo91000/archimedes/commit/d3ec037))
* chore(release): archimedes_crontab_runner@0.4.0 ([6ec53f6](https://github.com/leo91000/archimedes/commit/6ec53f6))
* chore(release): archimedes_crontab_parser@0.4.0 ([be084df](https://github.com/leo91000/archimedes/commit/be084df))
* chore(release): archimedes_shutdown_signal@0.2.2 ([d894609](https://github.com/leo91000/archimedes/commit/d894609))
* chore(release): archimedes_crontab_types@0.4.0 ([e82bf12](https://github.com/leo91000/archimedes/commit/e82bf12))
* chore(shutdown_signal): Use tokio macros features ([674f1ec](https://github.com/leo91000/archimedes/commit/674f1ec))
* chore(release): archimedes@0.1.0 ([4b2b809](https://github.com/leo91000/archimedes/commit/4b2b809))
* chore(release): archimedes_migrations@0.1.1 ([c10a4d2](https://github.com/leo91000/archimedes/commit/c10a4d2))
* chore(release): archimedes_crontab_runner@0.2.0 ([7d03c47](https://github.com/leo91000/archimedes/commit/7d03c47))
* chore(release): archimedes_crontab_parser@0.2.0 ([06f7db2](https://github.com/leo91000/archimedes/commit/06f7db2))
* chore(release): archimedes_shutdown_signal@0.1.1 ([a1c332a](https://github.com/leo91000/archimedes/commit/a1c332a))
* chore(release): archimedes_crontab_types@0.2.0 ([a910b4f](https://github.com/leo91000/archimedes/commit/a910b4f))
* chore(deps): update rust crate serde to 1.0.158 ([4643dcb](https://github.com/leo91000/archimedes/commit/4643dcb))
* chore(deps): update all non-major dependencies ([629a3dd](https://github.com/leo91000/archimedes/commit/629a3dd))
* chore(deps): update rust crate serde to 1.0.156 ([86685ec](https://github.com/leo91000/archimedes/commit/86685ec))
* chore(deps): update rust crate chrono to 0.4.24 ([5676a7c](https://github.com/leo91000/archimedes/commit/5676a7c))
* chore(deps): update rust crate serde to 1.0.155 ([f98fde2](https://github.com/leo91000/archimedes/commit/f98fde2))
* chore(deps): update rust crate futures to 0.3.27 ([2163377](https://github.com/leo91000/archimedes/commit/2163377))
* chore(deps): update rust crate serde to 1.0.154 ([abddcf5](https://github.com/leo91000/archimedes/commit/abddcf5))
* chore(deps): update rust crate serde to 1.0.153 ([374f207](https://github.com/leo91000/archimedes/commit/374f207))
* chore(deps): update rust crate thiserror to 1.0.39 ([0790a3f](https://github.com/leo91000/archimedes/commit/0790a3f))
* chore(deps): update rust crate serde_json to 1.0.94 ([a969360](https://github.com/leo91000/archimedes/commit/a969360))
* chore(deps): update rust crate serde_qs to 0.12.0 ([94df8c9](https://github.com/leo91000/archimedes/commit/94df8c9))
* chore(deps): update rust crate tokio to 1.26.0 ([9d8cd06](https://github.com/leo91000/archimedes/commit/9d8cd06))
* chore(deps): update rust crate once_cell to 1.17.1 ([9e9d8aa](https://github.com/leo91000/archimedes/commit/9e9d8aa))
* chore(deps): update rust crate serde_json to 1.0.93 ([7eb2054](https://github.com/leo91000/archimedes/commit/7eb2054))
* chore(deps): update all non-major dependencies ([3ecb31d](https://github.com/leo91000/archimedes/commit/3ecb31d))
* chore(deps): update rust crate futures to 0.3.26 ([8a89947](https://github.com/leo91000/archimedes/commit/8a89947))
* chore(deps): update rust crate tokio to 1.25.0 ([b28f180](https://github.com/leo91000/archimedes/commit/b28f180))
* chore(deps): update rust crate tokio to 1.24.2 ([035b7fa](https://github.com/leo91000/archimedes/commit/035b7fa))
* chore(deps): update rust crate nom to 7.1.3 ([5833efd](https://github.com/leo91000/archimedes/commit/5833efd))
* chore(deps): update rust crate serde_qs to 0.11.0 ([e6700bc](https://github.com/leo91000/archimedes/commit/e6700bc))
* chore(deps): update rust crate tokio to 1.24.1 ([9bc0335](https://github.com/leo91000/archimedes/commit/9bc0335))
* chore(deps): update rust crate tokio to 1.24.0 ([43ebddf](https://github.com/leo91000/archimedes/commit/43ebddf))
* chore(deps): update rust crate tokio to 1.23.1 ([d20c0b6](https://github.com/leo91000/archimedes/commit/d20c0b6))
* chore(deps): update rust crate nom to 7.1.2 ([038f4d2](https://github.com/leo91000/archimedes/commit/038f4d2))
* chore(deps): update rust crate once_cell to 1.17.0 ([c52bb61](https://github.com/leo91000/archimedes/commit/c52bb61))
* chore(deps): update rust crate serde to 1.0.152 ([a6115cb](https://github.com/leo91000/archimedes/commit/a6115cb))
* chore(deps): update rust crate num_cpus to 1.15.0 ([2a7ef10](https://github.com/leo91000/archimedes/commit/2a7ef10))
* chore: Fix error in code block in README ([3bf9f51](https://github.com/leo91000/archimedes/commit/3bf9f51))
* chore: Add differences with graphile-worker in README ([bce3367](https://github.com/leo91000/archimedes/commit/bce3367))
* chore: Add crontab runner to the worker main run function ([340445a](https://github.com/leo91000/archimedes/commit/340445a))
* chore: Add license field for crontab_runner toml file ([9266308](https://github.com/leo91000/archimedes/commit/9266308))
* chore: Specify version for workspace packages ([8e03f22](https://github.com/leo91000/archimedes/commit/8e03f22))
* chore: Remove keywords and categories from packages metadata ([798bded](https://github.com/leo91000/archimedes/commit/798bded))
* chore: Include https:// in package metadata documentation and homepage ([ebffd12](https://github.com/leo91000/archimedes/commit/ebffd12))
* chore: Add license to crontab_types ([3cd31cb](https://github.com/leo91000/archimedes/commit/3cd31cb))
* chore: Prepare packages for publishing ([5d99f5c](https://github.com/leo91000/archimedes/commit/5d99f5c))
* chore: Apply clippy lint ([66e3894](https://github.com/leo91000/archimedes/commit/66e3894))
* chore(deps): update all non-major dependencies ([bdb33af](https://github.com/leo91000/archimedes/commit/bdb33af))
* chore: Remove unused deps ([bb09685](https://github.com/leo91000/archimedes/commit/bb09685))
* chore: Remove nightly flag ([05b4c63](https://github.com/leo91000/archimedes/commit/05b4c63))
* chore: Allow dead code for temporarly unused function ([e5b4329](https://github.com/leo91000/archimedes/commit/e5b4329))
* chore: Clippy fixes ([506f98c](https://github.com/leo91000/archimedes/commit/506f98c))
* chore: Remove unused test ([5ac1deb](https://github.com/leo91000/archimedes/commit/5ac1deb))
* chore: Improve README ([d0434ce](https://github.com/leo91000/archimedes/commit/d0434ce))
* chore: Fix typo ([6557171](https://github.com/leo91000/archimedes/commit/6557171))
* chore: Add README ([5ab6f7f](https://github.com/leo91000/archimedes/commit/5ab6f7f))
* chore: Refactor folder structure ([ed29cec](https://github.com/leo91000/archimedes/commit/ed29cec))
* chore: Use u32 for crontab value ([956937e](https://github.com/leo91000/archimedes/commit/956937e))
* chore: Update schedule_crontab_jobs_at signature ([8d5e1e3](https://github.com/leo91000/archimedes/commit/8d5e1e3))
* chore: Refactor folder structure ([df41490](https://github.com/leo91000/archimedes/commit/df41490))
* chore: reexport error kind ([f6921a8](https://github.com/leo91000/archimedes/commit/f6921a8))
* chore: Remove unused regexes module ([c9cdf04](https://github.com/leo91000/archimedes/commit/c9cdf04))
* chore: Replace manual digit parsing with character::complete::u8 ([f5d680d](https://github.com/leo91000/archimedes/commit/f5d680d))
* chore: cleanup ([f7647a6](https://github.com/leo91000/archimedes/commit/f7647a6))
* chore: Remove empty file ([e748520](https://github.com/leo91000/archimedes/commit/e748520))
* chore: Refactor folder structure ([fdb0fc8](https://github.com/leo91000/archimedes/commit/fdb0fc8))
* chore: Remove unused Error ([ea794e6](https://github.com/leo91000/archimedes/commit/ea794e6))
* chore: Refactor worker ([2e06b42](https://github.com/leo91000/archimedes/commit/2e06b42))
* chore: Added example ([4273672](https://github.com/leo91000/archimedes/commit/4273672))
* chore: Clippy fixes ([db5ec81](https://github.com/leo91000/archimedes/commit/db5ec81))
* chore: extract escape_identifier ([cb423aa](https://github.com/leo91000/archimedes/commit/cb423aa))
* chore: Remove .env file ([edfc81c](https://github.com/leo91000/archimedes/commit/edfc81c))
* chore: Gitignore .env file ([4a61b9a](https://github.com/leo91000/archimedes/commit/4a61b9a))

### 🧪 Tests

* test: Add CrontabTimer tests ([a432ad8](https://github.com/leo91000/archimedes/commit/a432ad8))

### 📝 Docs

* docs: Added should_run_at documentation ([3b9fe96](https://github.com/leo91000/archimedes/commit/3b9fe96))

### 🤖 CI

* ci: Update release script ([3614a76](https://github.com/leo91000/archimedes/commit/3614a76))
* ci: Add renovate ([a78ffcf](https://github.com/leo91000/archimedes/commit/a78ffcf))
* ci: Remove all features flags on clippy check ([fb20e9b](https://github.com/leo91000/archimedes/commit/fb20e9b))
* ci: Add CI ([80d7fb5](https://github.com/leo91000/archimedes/commit/80d7fb5))

### 🚧 WIP

* wip: Adds signal handling ([a8d11b2](https://github.com/leo91000/archimedes/commit/a8d11b2))
* wip: first attempts at crontab_runner ([4c59b2e](https://github.com/leo91000/archimedes/commit/4c59b2e))
* wip: attempt at nom parsing crontab ([c48e972](https://github.com/leo91000/archimedes/commit/c48e972))
* wip: dynamic fn map ([8464613](https://github.com/leo91000/archimedes/commit/8464613))






