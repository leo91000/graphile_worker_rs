set dotenv-load

test:
  cargo test --all

test-docker:
  docker rm -f graphile-worker-rs-test 2> /dev/null || true
  docker run -d --name graphile-worker-rs-test -p 54233:5432 -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres postgres
  docker exec graphile-worker-rs-test bash -c 'while ! pg_isready -U postgres -h localhost; do sleep 1; done'
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' cargo test --all
  docker rm -f graphile-worker-rs-test

coverage-docker:
  docker rm -f graphile-worker-rs-test 2> /dev/null || true
  docker run -d --name graphile-worker-rs-test -p 54233:5432 -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres postgres
  docker exec graphile-worker-rs-test bash -c 'while ! pg_isready -U postgres -h localhost; do sleep 1; done'
  DATABASE_URL='postgres://postgres:postgres@localhost:54233/postgres' cargo tarpaulin --all --out Xml
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
