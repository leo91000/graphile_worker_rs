# Upstream migration fixture

`upstream-0.18.0-000020.sql` is copied verbatim from
[graphile/worker sql/000020.sql at 4cda192c5df254392a1dff350e5d73f7d2c18a85](https://github.com/graphile/worker/blob/4cda192c5df254392a1dff350e5d73f7d2c18a85/sql/000020.sql).
It is used only on isolated test databases to exercise the Node revision-20
upgrade shape. The repository's MIT license retains upstream's copyright notice.

## SQL retry faults

`sql_retry_faults.sql` injects two rolled-back serialization failures on claims
and two deadlocks on outcome persistence, observed through nontransactional
sequences. `tests/sql_retry.rs` uses it on isolated Rust databases. The October
22 upstream differential run uses the identical fixture against pinned Node
v0.18.0 source functions and all 20 official migrations. This fixture is for
disposable test databases only.
