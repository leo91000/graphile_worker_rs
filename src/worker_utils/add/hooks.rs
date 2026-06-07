use graphile_worker_lifecycle_hooks::{BeforeJobScheduleContext, JobScheduleResult};

use crate::{errors::GraphileWorkerError, JobSpec};

use super::super::client::WorkerUtils;

pub(super) async fn invoke_before_job_schedule(
    utils: &WorkerUtils,
    identifier: &str,
    payload: serde_json::Value,
    spec: &JobSpec,
) -> Result<serde_json::Value, GraphileWorkerError> {
    let Some(hooks) = &utils.hooks else {
        return Ok(payload);
    };

    let ctx = BeforeJobScheduleContext {
        identifier: identifier.to_string(),
        payload,
        spec: spec.clone(),
    };

    match hooks.intercept(ctx).await {
        JobScheduleResult::Continue(payload) => Ok(payload),
        JobScheduleResult::Skip => Err(GraphileWorkerError::JobScheduleSkipped),
        JobScheduleResult::Fail(msg) => Err(GraphileWorkerError::JobScheduleFailed(msg)),
    }
}
