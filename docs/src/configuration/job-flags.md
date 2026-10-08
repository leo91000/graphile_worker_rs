# Job flag routing

Job flags are string labels attached through `JobSpec::flags`. A worker can use
them to select or exclude jobs without changing the task identifier.

```rust,ignore
use graphile_worker::WorkerOptions;

let worker = WorkerOptions::default()
    .add_accepted_flag("linux")
    .add_accepted_flag("macos")
    .add_forbidden_flag("gpu")
    // ... database and task handlers
    .init()
    .await?;
```

This worker requires at least one of `linux` or `macos`. Any `gpu` flag vetoes
the job, even when another flag matches acceptance. Labels match exactly and
case-sensitively; `linux` and `LINUX` are different labels.

| Job flags | Eligible for this worker |
| --- | --- |
| `linux` | Yes |
| `macos`, `other` | Yes |
| `linux`, `infrastructure_resilient` | Yes |
| `linux`, `gpu` | No |
| `windows` | No |
| No flags or an empty flag list | No |

An empty accepted set adds no positive restriction. With no accepted flags,
untagged jobs remain eligible and only forbidden flags exclude work. Defaults
therefore preserve existing behavior.

These are per-worker routing rules. An unfiltered worker can still claim flagged
jobs if it registers their task handler. Configure every worker that must be
restricted. Matching an accepted label does not establish support for every
other label on a job; accepted flags are not a capability-requirement system.

Filtering happens before jobs are claimed. A rejected job stays queued with its
attempt count and ownership unchanged. Jobs without an eligible worker remain
queued until a suitable worker is available.

When `.local_queue(...)` is configured, continuous workers retain batching with
either accepted or forbidden flags. Each local queue applies the worker's filters
in PostgreSQL before claiming a batch; rejected jobs never enter the cache.
Workers without local queue configuration use direct claims. Both continuous
execution and `run_once` apply the same rules, including follow-up jobs in named
queues. TTL expiry and shutdown return unused prefetched jobs as usual.

The existing `get_job`, `batch_get_jobs`, and `job_stream` APIs retain their
forbidden-only signatures. Direct callers needing positive filtering can use
`get_job_with_filter`, `batch_get_jobs_with_filter`, or `job_stream_with_filter`
with `JobFlagFilter`. `JobFlagFilter::new` borrows label sets, while
`JobFlagFilter::owned` owns them for streams or other longer-lived uses.
