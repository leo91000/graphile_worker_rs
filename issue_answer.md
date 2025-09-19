Short answer: yes.

You’ve got two options, depending on whether you want a fixed delay or are OK with exponential backoff.

- Fixed delay (recommended): schedule a follow‑up job yourself with run_at
  - Create/queue the “check” job (or the same job) with run_at = now + 10s.
  - Use a job_key + JobKeyMode::Replace to avoid duplicates.

Example (from app code where you have a Worker and can get utils):
- `utils.add_job(...)` schedules a job at a specific time.

```rust
use chrono::{Utc, Duration};
use graphile_worker::{JobSpecBuilder, JobKeyMode, Worker};

let utils = worker.create_utils();

utils.add_job(
    CheckWs { request_id },
    JobSpecBuilder::new()
        .run_at(Utc::now() + Duration::seconds(10))
        .job_key(format!("ws-check:{request_id}"))
        .job_key_mode(JobKeyMode::Replace)
        .build(),
).await?;
```

If you need to queue from inside the task handler (using the new ctx helpers):
- Import `WorkerContextExt` and call `ctx.add_job(...)` directly.

```rust
use chrono::{Utc, Duration};
use graphile_worker::{
    TaskHandler, WorkerContext, WorkerContextExt, IntoTaskHandlerResult,
    JobSpecBuilder, JobKeyMode,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendWs { request_id: String /* ... */ }

#[derive(Deserialize, Serialize)]
struct CheckWs { request_id: String }

impl TaskHandler for SendWs {
    const IDENTIFIER: &'static str = "send_ws";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        // 1) send the websocket message...

        // 2) schedule a follow-up check in 10s using ctx helpers
        ctx.add_job(
            CheckWs { request_id: self.request_id.clone() },
            JobSpecBuilder::new()
                .run_at(Utc::now() + Duration::seconds(10))
                .job_key(format!("ws-check:{}", self.request_id))
                .job_key_mode(JobKeyMode::Replace)
                .build(),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok::<(), String>(())
    }
}
```

- Exponential backoff retry (not fixed 10s): return an error
  - If your handler returns `Err`, the worker reschedules with exponential delay: `run_at = max(now, run_at) + exp(min(attempts, 10))` seconds. That’s ~2.7s, 7.4s, 20s, 54s… — not a precise 10 seconds.

Summary:
- Precise 10s: schedule a new/follow‑up job with `run_at`.
- “Just retry later”: error out and let the built‑in exponential backoff handle it.
