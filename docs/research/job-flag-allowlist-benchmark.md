# Job-flag allowlist claim benchmark

Measured on PostgreSQL 14.24 and 18.0 with both SQLx and tokio-postgres. The manual test is tests/fetch_benchmark.rs::benchmark_job_flag_allowlist. This is a local synthetic workload, not a production SLO or a concurrency benchmark.

## Method

Each isolated database uses the normal worker migrations and claim indexes. Seed 100,000 available jobs with deterministic run_at order. Place rejected jobs at the front: 10%, 90%, or 99.9% of the backlog. Use 1/4/16 flags per job and 1/8/64 allowlist entries respectively. Flags-per-job and allowlist size vary together, so their independent effects are not isolated. Compare accepted, forbidden and allowlist predicates that select exactly the same seeded jobs.

Call the actual single-job or batch-of-10 claim function through each driver, verify eligibility, and return ownership after each claim. Rotate policy order across 15 rounds, discard three warmups, then report the median and empirical p95 of 12 samples. Timings include the claim round trip and Rust parameter/result handling; return_jobs is outside the timed interval. These warm-cache samples do not establish production tail latency. The host also runs other workloads.

## Long rejected prefix

For single claims with 99,900 rejected jobs ahead of 100 eligible jobs:

| Server / driver | Job flags / allowlist size | Allowlist median ms | Empirical p95 ms | Forbidden median ms |
| --- | --- | --- | --- | --- |
| pg14-sqlx | 1 / 1 | 34.062 | 35.047 | 15.675 |
| pg14-sqlx | 4 / 8 | 40.632 | 43.873 | 16.711 |
| pg14-sqlx | 16 / 64 | 76.071 | 94.964 | 17.274 |
| pg18-sqlx | 1 / 1 | 42.478 | 63.653 | 15.585 |
| pg18-sqlx | 4 / 8 | 49.877 | 67.886 | 16.242 |
| pg18-sqlx | 16 / 64 | 87.315 | 99.422 | 17.454 |
| pg14-tokio | 1 / 1 | 36.192 | 41.447 | 20.272 |
| pg14-tokio | 4 / 8 | 42.095 | 52.313 | 18.075 |
| pg14-tokio | 16 / 64 | 76.164 | 80.985 | 18.157 |
| pg18-tokio | 1 / 1 | 42.602 | 47.830 | 16.882 |
| pg18-tokio | 4 / 8 | 51.011 | 57.414 | 17.998 |
| pg18-tokio | 16 / 64 | 89.937 | 97.887 | 19.144 |

Allowlisting costs more per examined row than checking one excluded label. The long-prefix medians were approximately 34–90 ms, versus 16–20 ms for forbidden filtering. The filter is opt-in; an unset allowlist generates the existing SQL and bindings, pinned by a regression test. Use accepted routing and appropriate task registration to narrow candidates, and isolate task identifiers when a small worker pool would otherwise scan a large unrelated backlog.

## Predicate comparison

The guarded JSONB subtraction predicate preserves key-only semantics, including false or nested object values, and rejects non-object input safely. An equivalent guarded NOT EXISTS query using jsonb_object_keys plus text-array membership was also measured on PG14 / SQLx. For the 99.9% rejected-prefix, 16-key/64-entry, batch-of-10 case, subtraction was 77.147 ms median while enumeration was 368.007 ms. Keep subtraction. This is an observed comparison, not a universal guarantee about all distributions.

The alternative replaced only the enabled allowlist clause in
`JobFlagFilter::clause` with this template, retaining the same parameter bindings
and the same manual benchmark. `$N` denotes the allowlist parameter ordinal:

```sql
AND CASE
      WHEN jobs.flags IS NULL THEN true
      WHEN jsonb_typeof(jobs.flags) = 'object' THEN NOT EXISTS (
        SELECT 1 FROM jsonb_object_keys(jobs.flags) AS flag
        WHERE NOT (flag = ANY($N::text[]))
      )
      ELSE false
    END
```

## Reproduce

Set DATABASE_URL to a disposable local PostgreSQL server; the helper creates and drops only its own UUID database.

~~~sh
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
  cargo test --test fetch_benchmark benchmark_job_flag_allowlist -- --ignored --nocapture

DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
  cargo test --no-default-features --features runtime-tokio,driver-tokio-postgres \
  --test fetch_benchmark benchmark_job_flag_allowlist -- --ignored --nocapture
~~~

No schema or index changes were made. Concurrent throughput, cold-cache behavior, and production workload budgets remain unmeasured.
