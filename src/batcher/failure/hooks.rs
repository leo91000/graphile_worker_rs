use std::sync::Arc;

use graphile_worker_lifecycle_hooks::{HookRegistry, JobFailContext, JobPermanentlyFailContext};

use super::FailureRequest;

pub(super) async fn emit_failure_hook(
    req: &FailureRequest,
    worker_id: &str,
    hooks: &Arc<HookRegistry>,
) {
    if hooks.is_empty() {
        return;
    }

    if req.will_retry {
        hooks
            .emit(JobFailContext {
                job: req.job.clone(),
                worker_id: worker_id.to_string(),
                error: req.error.clone(),
                will_retry: true,
            })
            .await;
    } else {
        hooks
            .emit(JobPermanentlyFailContext {
                job: req.job.clone(),
                worker_id: worker_id.to_string(),
                error: req.error.clone(),
            })
            .await;
    }
}
