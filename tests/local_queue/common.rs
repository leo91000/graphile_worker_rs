use graphile_worker::{
    HookRegistry, IntoTaskHandlerResult, JobSpec, LocalQueueConfig, LocalQueueInit, Plugin,
    RefetchDelayConfig, TaskHandler, Worker, WorkerContext,
};
use graphile_worker_runtime::sleep as runtime_sleep;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    task::spawn_local,
    time::{sleep, Instant},
};

use crate::helpers::{with_test_db, StaticCounter};

#[derive(Serialize, Deserialize)]
struct LocalQueueJob {
    id: u32,
}

static LOCAL_QUEUE_JOB_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for LocalQueueJob {
    const IDENTIFIER: &'static str = "local_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        LOCAL_QUEUE_JOB_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct BatchJob {
    id: u32,
}

static BATCH_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for BatchJob {
    const IDENTIFIER: &'static str = "batch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        BATCH_CALL_COUNT.increment().await;
    }
}

#[derive(Clone)]
struct LocalQueueInitCounterPlugin {
    counter: Arc<AtomicU32>,
}

impl Plugin for LocalQueueInitCounterPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(LocalQueueInit, move |_ctx| {
            let counter = self.counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}

#[derive(Serialize, Deserialize)]
struct MultiLocalQueueJob {
    id: u32,
}

static MULTI_LOCAL_QUEUE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for MultiLocalQueueJob {
    const IDENTIFIER: &'static str = "multi_local_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        MULTI_LOCAL_QUEUE_CALL_COUNT.increment().await;
    }
}




#[derive(Serialize, Deserialize)]
struct ShutdownJob {
    id: u32,
}

static SHUTDOWN_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ShutdownJob {
    const IDENTIFIER: &'static str = "shutdown_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_secs(10)).await;
        SHUTDOWN_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct FlaggedJob {
    id: u32,
}

static FLAGGED_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for FlaggedJob {
    const IDENTIFIER: &'static str = "flagged_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        FLAGGED_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct RunOnceJob {
    id: u32,
}

static RUN_ONCE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RunOnceJob {
    const IDENTIFIER: &'static str = "run_once_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        RUN_ONCE_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct TtlExpiryJob {
    id: u32,
}

static TTL_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for TtlExpiryJob {
    const IDENTIFIER: &'static str = "ttl_expiry_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_secs(30)).await;
        TTL_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct RefetchDelayJob {
    id: u32,
}

static REFETCH_DELAY_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RefetchDelayJob {
    const IDENTIFIER: &'static str = "refetch_delay_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        REFETCH_DELAY_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct SmallQueueJob {
    id: u32,
}

static SMALL_QUEUE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for SmallQueueJob {
    const IDENTIFIER: &'static str = "small_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        SMALL_QUEUE_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct ConcurrentDistributionJob {
    id: u32,
}

static CONCURRENT_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ConcurrentDistributionJob {
    const IDENTIFIER: &'static str = "concurrent_distribution_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_millis(100)).await;
        CONCURRENT_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct ModeTransitionJob {
    id: u32,
}

static MODE_TRANSITION_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ModeTransitionJob {
    const IDENTIFIER: &'static str = "mode_transition_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_millis(50)).await;
        MODE_TRANSITION_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct EmptyQueueJob {
    id: u32,
}

static EMPTY_QUEUE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for EmptyQueueJob {
    const IDENTIFIER: &'static str = "empty_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        EMPTY_QUEUE_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct LargeBatchJob {
    id: u32,
}

static LARGE_BATCH_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for LargeBatchJob {
    const IDENTIFIER: &'static str = "large_batch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        LARGE_BATCH_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct RefetchDelayWithJobsJob {
    id: u32,
}

static REFETCH_WITH_JOBS_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RefetchDelayWithJobsJob {
    const IDENTIFIER: &'static str = "refetch_delay_with_jobs_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        REFETCH_WITH_JOBS_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct PulseImmediateFetchJob {
    id: u32,
}

static PULSE_IMMEDIATE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for PulseImmediateFetchJob {
    const IDENTIFIER: &'static str = "pulse_immediate_fetch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        PULSE_IMMEDIATE_CALL_COUNT.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct ReleaseWaitsJob {
    id: u32,
}

static RELEASE_WAITS_CALL_COUNT: StaticCounter = StaticCounter::new();
static RELEASE_WAITS_COMPLETED: StaticCounter = StaticCounter::new();

impl TaskHandler for ReleaseWaitsJob {
    const IDENTIFIER: &'static str = "release_waits_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        RELEASE_WAITS_CALL_COUNT.increment().await;
        runtime_sleep(Duration::from_millis(200)).await;
        RELEASE_WAITS_COMPLETED.increment().await;
    }
}


#[derive(Serialize, Deserialize)]
struct RefetchAbortJob {
    id: u32,
}

static REFETCH_ABORT_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RefetchAbortJob {
    const IDENTIFIER: &'static str = "refetch_abort_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        REFETCH_ABORT_CALL_COUNT.increment().await;
    }
}
