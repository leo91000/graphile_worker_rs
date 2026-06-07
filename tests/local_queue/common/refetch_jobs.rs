use super::*;

#[derive(Serialize, Deserialize)]
pub(super) struct RefetchDelayJob {
    pub(super) id: u32,
}

pub(super) static REFETCH_DELAY_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RefetchDelayJob {
    const IDENTIFIER: &'static str = "refetch_delay_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        REFETCH_DELAY_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct SmallQueueJob {
    pub(super) id: u32,
}

pub(super) static SMALL_QUEUE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for SmallQueueJob {
    const IDENTIFIER: &'static str = "small_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        SMALL_QUEUE_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct RefetchDelayWithJobsJob {
    pub(super) id: u32,
}

pub(super) static REFETCH_WITH_JOBS_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RefetchDelayWithJobsJob {
    const IDENTIFIER: &'static str = "refetch_delay_with_jobs_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        REFETCH_WITH_JOBS_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct PulseImmediateFetchJob {
    pub(super) id: u32,
}

pub(super) static PULSE_IMMEDIATE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for PulseImmediateFetchJob {
    const IDENTIFIER: &'static str = "pulse_immediate_fetch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        PULSE_IMMEDIATE_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct RefetchAbortJob {
    pub(super) id: u32,
}

pub(super) static REFETCH_ABORT_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RefetchAbortJob {
    const IDENTIFIER: &'static str = "refetch_abort_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        REFETCH_ABORT_CALL_COUNT.increment().await;
    }
}
