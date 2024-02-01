set dotenv-load

test:
  cargo test --all

doc-test:
  cargo test --all --doc

fmt:
  cargo fmt --all

check-fmt:
  cargo fmt --all -- --check

check-clippy:
  cargo clippy --all -- -D warnings
