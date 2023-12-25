fmt:
  cargo fmt --all

check-fmt:
  cargo fmt --all -- --check

check-clippy:
  cargo clippy --all -- -D warnings
