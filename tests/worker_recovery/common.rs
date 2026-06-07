use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker::recovery::job_has_resilient_flag;
use graphile_worker::sql::{
    recover_workers::{get_locked_jobs_for_recovery, recover_dead_worker_jobs},
    return_jobs::recovery::return_job_for_recovery,
    worker_heartbeat::{
        lock::try_acquire_sweep_lock,
        registration::{worker_deregister, worker_heartbeat},
        stale::{
        delete_stale_workers, get_worker_last_heartbeat, list_orphan_locked_workers,
            list_stale_workers, worker_holds_resilient_locks,
        },
    },
};
use graphile_worker::{
    FailureReason, HookRegistry, IntoTaskHandlerResult, Job, JobRecovery, JobRecoveryResult,
    JobSpec, SweepStaleWorkersOptions, TaskHandler, Worker, WorkerContext, WorkerRecoveryConfig,
    WorkerUtils, INFRASTRUCTURE_RESILIENT_FLAG,
};
use graphile_worker_runtime::sleep as runtime_sleep;
use indoc::formatdoc;
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Instant};

use helpers::sql::safe_query;
use helpers::with_test_db;



#[derive(Deserialize, Serialize)]
struct LongJob {
    id: i64,
}

impl TaskHandler for LongJob {
    const IDENTIFIER: &'static str = "long_heartbeat_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_secs(120)).await;
        Ok::<(), String>(())
    }
}
