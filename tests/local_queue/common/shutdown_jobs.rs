use super::*;

#[derive(Serialize, Deserialize)]
pub(super) struct ShutdownJob {
    pub(super) id: u32,
}

pub(super) static SHUTDOWN_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ShutdownJob {
    const IDENTIFIER: &'static str = "shutdown_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_secs(10)).await;
        SHUTDOWN_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct TtlExpiryJob {
    pub(super) id: u32,
}

pub(super) static TTL_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for TtlExpiryJob {
    const IDENTIFIER: &'static str = "ttl_expiry_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_secs(30)).await;
        TTL_CALL_COUNT.increment().await;
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct ReleaseWaitsJob {
    pub(super) id: u32,
}

pub(super) static RELEASE_WAITS_CALL_COUNT: StaticCounter = StaticCounter::new();
pub(super) static RELEASE_WAITS_COMPLETED: StaticCounter = StaticCounter::new();

impl TaskHandler for ReleaseWaitsJob {
    const IDENTIFIER: &'static str = "release_waits_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        RELEASE_WAITS_CALL_COUNT.increment().await;
        runtime_sleep(Duration::from_millis(200)).await;
        RELEASE_WAITS_COMPLETED.increment().await;
    }
}
