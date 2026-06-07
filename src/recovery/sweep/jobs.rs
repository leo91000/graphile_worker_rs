use std::sync::Arc;
use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, Schema};
use graphile_worker_lifecycle_hooks::{FailureReason, HookRegistry, JobInterruptedContext};

use crate::errors::GraphileWorkerError;
use crate::sql::recover_workers::{get_locked_jobs_for_recovery, recover_dead_worker_jobs};

use super::super::job_recovery::apply_job_recovery;
use super::super::types::JobRecoveryRequest;

pub(super) async fn recover_jobs_from_workers(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    hooks: Option<&Arc<HookRegistry>>,
    worker_id: &str,
    worker_ids: &[String],
    recovery_delay: Duration,
) -> Result<i32, GraphileWorkerError> {
    if worker_ids.is_empty() {
        return Ok(0);
    }

    let has_hooks = hooks.is_some_and(|hooks| !hooks.is_empty());
    if !has_hooks {
        return recover_dead_worker_jobs(&mut executor, schema, worker_ids, recovery_delay).await;
    }

    let jobs = get_locked_jobs_for_recovery(&mut executor, schema, worker_ids).await?;
    let mut recovered_count = 0;

    for job in jobs {
        let Some(previous_worker_id) = job.locked_by().as_deref() else {
            continue;
        };

        let outcome = apply_job_recovery(
            &mut executor,
            schema,
            JobRecoveryRequest {
                hooks,
                worker_id,
                job: job.clone(),
                previous_worker_id,
                reason: FailureReason::WorkerCrashed,
                recovery_delay,
            },
        )
        .await?;

        if outcome.was_handled() {
            recovered_count += 1;
            if let Some(hooks) = hooks {
                hooks
                    .emit(JobInterruptedContext {
                        job,
                        worker_id: worker_id.to_string(),
                        reason: FailureReason::WorkerCrashed,
                    })
                    .await;
            }
        }
    }

    Ok(recovered_count)
}
