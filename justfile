set dotenv-load

test:
  cargo test --all

test-runtime runtime="runtime-tokio" driver="driver-sqlx":
  #!/usr/bin/env bash
  set -euo pipefail
  if [ -z "${DATABASE_URL:-}" ]; then
    just test-docker-runtime '{{runtime}}' '{{driver}}'
    exit 0
  fi
  features='{{runtime}},{{driver}}'
  target_args=(--lib --tests)
  test_args=()
  case '{{driver}}' in
    driver-sqlx)
      features="${features},tls-rustls"
      target_args=(--all)
      ;;
    driver-tokio-postgres)
      if [ '{{runtime}}' != 'runtime-tokio' ]; then
        echo "driver-tokio-postgres only supports runtime-tokio tests" >&2
        exit 1
      fi
      test_args=(-- --test-threads=1)
      ;;
    *)
      echo "unknown postgres driver: {{driver}}" >&2
      exit 1
      ;;
  esac
  cargo test "${target_args[@]}" --no-default-features --features "${features}" "${test_args[@]}"

test-docker:
  docker rm -f graphile-worker-rs-test 2> /dev/null || true
  docker run -d --name graphile-worker-rs-test -p 54233:5432 -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres postgres
  docker exec graphile-worker-rs-test bash -c 'for i in $(seq 30); do pg_isready -U postgres -h localhost && exit 0; sleep 1; done; echo "Postgres not ready after 30s" >&2; exit 1'
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' cargo test --all
  docker rm -f graphile-worker-rs-test

test-docker-runtime runtime="runtime-tokio" driver="driver-sqlx":
  #!/usr/bin/env bash
  set -euo pipefail
  docker rm -f graphile-worker-rs-test 2> /dev/null || true
  docker run -d --name graphile-worker-rs-test -p 54233:5432 -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres postgres
  trap 'docker rm -f graphile-worker-rs-test' EXIT
  docker exec graphile-worker-rs-test bash -c 'for i in $(seq 30); do pg_isready -U postgres -h localhost && exit 0; sleep 1; done; echo "Postgres not ready after 30s" >&2; exit 1'
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' just test-runtime '{{runtime}}' '{{driver}}'

test-docker-all-runtimes:
  #!/usr/bin/env bash
  set -euo pipefail
  docker rm -f graphile-worker-rs-test 2> /dev/null || true
  docker run -d --name graphile-worker-rs-test -p 54233:5432 -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres postgres
  trap 'docker rm -f graphile-worker-rs-test' EXIT
  docker exec graphile-worker-rs-test bash -c 'for i in $(seq 30); do pg_isready -U postgres -h localhost && exit 0; sleep 1; done; echo "Postgres not ready after 30s" >&2; exit 1'
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' just test-runtime runtime-tokio driver-sqlx
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' just test-runtime runtime-async-std driver-sqlx

test-docker-all-matrices:
  #!/usr/bin/env bash
  set -euo pipefail
  docker rm -f graphile-worker-rs-test 2> /dev/null || true
  docker run -d --name graphile-worker-rs-test -p 54233:5432 -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres postgres
  trap 'docker rm -f graphile-worker-rs-test' EXIT
  docker exec graphile-worker-rs-test bash -c 'for i in $(seq 30); do pg_isready -U postgres -h localhost && exit 0; sleep 1; done; echo "Postgres not ready after 30s" >&2; exit 1'
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' just test-runtime runtime-tokio driver-sqlx
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' just test-runtime runtime-async-std driver-sqlx
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' just test-runtime runtime-tokio driver-tokio-postgres

coverage-docker:
  docker rm -f graphile-worker-rs-test 2> /dev/null || true
  docker run -d --name graphile-worker-rs-test -p 54233:5432 -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres postgres
  docker exec graphile-worker-rs-test bash -c 'for i in $(seq 30); do pg_isready -U postgres -h localhost && exit 0; sleep 1; done; echo "Postgres not ready after 30s" >&2; exit 1'
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' cargo tarpaulin --all --out Xml --timeout 300 -- --test-threads=1
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' cargo tarpaulin --all --out Xml --no-default-features --features driver-tokio-postgres --timeout 300 -- --test-threads=1
  docker rm -f graphile-worker-rs-test

coverage-diff branch="main":
  just coverage-docker
  diff-cover cobertura.xml --compare-branch={{branch}}

doc-test:
  cargo test --all --doc

fmt:
  cargo fmt --all

check-fmt:
  cargo fmt --all -- --check

check:
  cargo check --all --all-targets

check-clippy:
  cargo clippy --all --all-targets -- -D warnings

lint: fmt check check-clippy

bench:
  cargo bench

bench-docker:
  docker rm -f graphile-worker-rs-bench 2> /dev/null || true
  docker run -d --name graphile-worker-rs-bench -p 54233:5432 -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres postgres
  docker exec graphile-worker-rs-bench bash -c 'while ! pg_isready -U postgres -h localhost; do sleep 1; done'
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' cargo bench
  docker rm -f graphile-worker-rs-bench
