# Changelog

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





