use super::*;

#[derive(Serialize, Deserialize)]
pub(super) struct LocalQueueJob {
    pub(super) id: u32,
}

pub(super) static LOCAL_QUEUE_JOB_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for LocalQueueJob {
    const IDENTIFIER: &'static str = "local_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        LOCAL_QUEUE_JOB_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct BatchJob {
    pub(super) id: u32,
}

pub(super) static BATCH_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for BatchJob {
    const IDENTIFIER: &'static str = "batch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        BATCH_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct MultiLocalQueueJob {
    pub(super) id: u32,
}

pub(super) static MULTI_LOCAL_QUEUE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for MultiLocalQueueJob {
    const IDENTIFIER: &'static str = "multi_local_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        MULTI_LOCAL_QUEUE_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct FlaggedJob {
    pub(super) id: u32,
}

pub(super) static FLAGGED_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for FlaggedJob {
    const IDENTIFIER: &'static str = "flagged_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        FLAGGED_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct RunOnceJob {
    pub(super) id: u32,
}

pub(super) static RUN_ONCE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RunOnceJob {
    const IDENTIFIER: &'static str = "run_once_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        RUN_ONCE_CALL_COUNT.increment().await;
    }
}
