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

## Job flag allowlist

An allowlist rejects a job carrying any flag key outside the configured set. This
is useful for build machines: new labels such as `kvm` should not become eligible
until the operator explicitly permits them on the appropriate workers.

```rust,ignore
let worker = WorkerOptions::default()
    .job_flag_allowlist(["amd-linux", "kvm", "fast-disk", "infrastructure_resilient"])
    .add_accepted_flag("fast-disk")
    // ... database and task handlers
    .init()
    .await?;
```

This worker reserves itself for `fast-disk` jobs and rejects unknown labels. A
job tagged `amd-linux`, `fast-disk`, and `kvm` is eligible; adding an unlisted
`arm-linux` label makes it ineligible. Include operational labels explicitly.
The allowlist is a closed label filter, not automatic validation of capabilities.

| Job flags | Allowlist `amd-linux`, `kvm` | Same allowlist + accept `kvm` |
| --- | --- | --- |
| `amd-linux` | Yes | No |
| `amd-linux`, `kvm` | Yes | Yes |
| `amd-linux`, `kvm`, `gpu` | No | No |
| No flags / empty object | Yes | No |

Leaving the allowlist unset preserves existing filtering. Passing an empty list
permits only untagged jobs for this predicate, and accepted and forbidden flags
still apply. Typed cron currently creates untagged jobs. Labels match exactly and
case-sensitively, duplicates have no effect, and JSON object values are ignored.
Raw SQL arrays, scalars and JSON `null` are rejected only when an allowlist is
enabled; SQL NULL is untagged.

At least one accepted label must also be allowlisted to satisfy both positive
filters. An empty allowlist combined with nonempty accepted flags admits no jobs.
Workers warn at initialization when these two sets have no common label; a
partially overlapping accepted set is valid because accepted routing is any-match.

Allowlisting applies before direct and LocalQueue batch claims. It does not
retroactively filter already claimed jobs. Configure every worker sharing the task
before relying on this rule: gracefully drain old or unconfigured workers and
their prefetched work, or use a new task identifier that only configured workers
register. Jobs with no eligible worker remain queued without consuming attempts.

Query and stream callers can attach an owned or borrowed list using
`JobFlagFilter::with_allowlist`, and inspect it with `allowlist()`. Existing
constructors and forbidden-only function signatures remain supported.

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
