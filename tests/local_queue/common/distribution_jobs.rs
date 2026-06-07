use super::*;

#[derive(Serialize, Deserialize)]
pub(super) struct ConcurrentDistributionJob {
    pub(super) id: u32,
}

pub(super) static CONCURRENT_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ConcurrentDistributionJob {
    const IDENTIFIER: &'static str = "concurrent_distribution_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_millis(100)).await;
        CONCURRENT_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct ModeTransitionJob {
    pub(super) id: u32,
}

pub(super) static MODE_TRANSITION_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ModeTransitionJob {
    const IDENTIFIER: &'static str = "mode_transition_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_millis(50)).await;
        MODE_TRANSITION_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct EmptyQueueJob {
    pub(super) id: u32,
}

pub(super) static EMPTY_QUEUE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for EmptyQueueJob {
    const IDENTIFIER: &'static str = "empty_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        EMPTY_QUEUE_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct LargeBatchJob {
    pub(super) id: u32,
}

pub(super) static LARGE_BATCH_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for LargeBatchJob {
    const IDENTIFIER: &'static str = "large_batch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        LARGE_BATCH_CALL_COUNT.increment().await;
    }
}
