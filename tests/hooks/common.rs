use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;

use graphile_worker::{
    AfterJobRun, BeforeJobRun, BeforeJobSchedule, HookRegistry, HookResult, IntoTaskHandlerResult,
    JobComplete, JobFail, JobFetch, JobPermanentlyFail, JobScheduleResult, JobSpec, JobStart,
    LocalQueueConfig, LocalQueueGetJobsComplete, LocalQueueInit, LocalQueueMode,
    LocalQueueRefetchDelayExpired, LocalQueueRefetchDelayStart, LocalQueueReturnJobs,
    LocalQueueSetMode, Plugin, RefetchDelayConfig, TaskHandler, Worker, WorkerContext, WorkerInit,
    WorkerShutdown, WorkerStart,
};
use graphile_worker_runtime::sleep as runtime_sleep;
use serde::{Deserialize, Serialize};
use tokio::{
    task::spawn_local,
    time::{sleep, Duration, Instant},
};

use crate::helpers::with_test_db;

#[derive(Debug, Default)]
struct HookCounters {
    worker_init: AtomicU32,
    worker_start: AtomicU32,
    worker_shutdown: AtomicU32,
    job_fetch: AtomicU32,
    job_start: AtomicU32,
    job_complete: AtomicU32,
    job_fail: AtomicU32,
    before_job_run: AtomicU32,
    skipped: AtomicU32,
    failed_by_hook: AtomicU32,
}

#[derive(Clone)]
struct TestHooksPlugin {
    counters: Arc<HookCounters>,
}

impl TestHooksPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(HookCounters::default()),
        }
    }

    fn counters(&self) -> Arc<HookCounters> {
        self.counters.clone()
    }
}

impl Plugin for TestHooksPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(WorkerInit, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.worker_init.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(WorkerStart, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.worker_start.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(WorkerShutdown, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.worker_shutdown.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobFetch, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.job_fetch.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobStart, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.job_start.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobComplete, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.job_complete.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobFail, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.job_fail.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(BeforeJobRun, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.before_job_run.fetch_add(1, Ordering::SeqCst);

                let should_skip = ctx
                    .payload
                    .get("skip")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let should_fail = ctx
                    .payload
                    .get("force_fail")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if should_skip {
                    counters.skipped.fetch_add(1, Ordering::SeqCst);
                    return HookResult::Skip;
                }

                if should_fail {
                    counters.failed_by_hook.fetch_add(1, Ordering::SeqCst);
                    return HookResult::Fail("Forced failure by hook".into());
                }

                HookResult::Continue
            }
        });
    }
}

#[derive(Serialize, Deserialize)]
struct TestJob {
    value: u32,
    #[serde(default)]
    skip: bool,
    #[serde(default)]
    force_fail: bool,
    #[serde(default)]
    should_error: bool,
}

impl TaskHandler for TestJob {
    const IDENTIFIER: &'static str = "test_hooks_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        if self.should_error {
            return Err("Task error".to_string());
        }
        Ok::<(), String>(())
    }
}

async fn wait_for_condition<F>(condition: F, timeout_secs: u64, msg: &str)
where
    F: Fn() -> bool,
{
    let start = Instant::now();
    while !condition() {
        if start.elapsed().as_secs() > timeout_secs {
            panic!("{}", msg);
        }
        sleep(Duration::from_millis(50)).await;
    }
}






#[derive(Debug, Default)]
struct ScheduleHookCounters {
    before_schedule: AtomicU32,
    transformed: AtomicU32,
    skipped: AtomicU32,
    failed: AtomicU32,
}

#[derive(Clone)]
struct ScheduleHooksPlugin {
    counters: Arc<ScheduleHookCounters>,
}

impl ScheduleHooksPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(ScheduleHookCounters::default()),
        }
    }

    fn counters(&self) -> Arc<ScheduleHookCounters> {
        self.counters.clone()
    }
}

impl Plugin for ScheduleHooksPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.before_schedule.fetch_add(1, Ordering::SeqCst);

                let should_skip = ctx
                    .payload
                    .get("skip_schedule")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let should_fail = ctx
                    .payload
                    .get("fail_schedule")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let should_transform = ctx
                    .payload
                    .get("transform")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if should_skip {
                    counters.skipped.fetch_add(1, Ordering::SeqCst);
                    return JobScheduleResult::Skip;
                }

                if should_fail {
                    counters.failed.fetch_add(1, Ordering::SeqCst);
                    return JobScheduleResult::Fail("Schedule failed by hook".into());
                }

                if should_transform {
                    counters.transformed.fetch_add(1, Ordering::SeqCst);
                    let mut new_payload = ctx.payload.clone();
                    if let Some(obj) = new_payload.as_object_mut() {
                        obj.insert("transformed".into(), serde_json::json!(true));
                    }
                    return JobScheduleResult::Continue(new_payload);
                }

                JobScheduleResult::Continue(ctx.payload)
            }
        });
    }
}






#[derive(Debug, Default)]
struct IdentifierCapturingCounters {
    captured_identifier: std::sync::Mutex<Option<String>>,
    captured_spec_queue: std::sync::Mutex<Option<String>>,
    captured_spec_priority: std::sync::Mutex<Option<i16>>,
}

#[derive(Clone)]
struct IdentifierCapturingPlugin {
    counters: Arc<IdentifierCapturingCounters>,
}

impl IdentifierCapturingPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(IdentifierCapturingCounters::default()),
        }
    }

    fn counters(&self) -> Arc<IdentifierCapturingCounters> {
        self.counters.clone()
    }
}

impl Plugin for IdentifierCapturingPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let counters = counters.clone();
            async move {
                *counters.captured_identifier.lock().unwrap() = Some(ctx.identifier.clone());
                *counters.captured_spec_queue.lock().unwrap() = ctx.spec.queue_name().clone();
                *counters.captured_spec_priority.lock().unwrap() = *ctx.spec.priority();
                JobScheduleResult::Continue(ctx.payload)
            }
        });
    }
}


#[derive(Serialize, Deserialize)]
struct TypedScheduleJob {
    message: String,
    #[serde(default)]
    transform: bool,
}

impl TaskHandler for TypedScheduleJob {
    const IDENTIFIER: &'static str = "typed_schedule_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}


#[derive(Debug, Default)]
struct ChainedTransformCounters {
    plugin1_calls: AtomicU32,
    plugin2_calls: AtomicU32,
}

#[derive(Clone)]
struct ChainedTransformPlugin1 {
    counters: Arc<ChainedTransformCounters>,
}

impl Plugin for ChainedTransformPlugin1 {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.plugin1_calls.fetch_add(1, Ordering::SeqCst);
                let mut payload = ctx.payload.clone();
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("plugin1_processed".into(), serde_json::json!(true));
                }
                JobScheduleResult::Continue(payload)
            }
        });
    }
}

#[derive(Clone)]
struct ChainedTransformPlugin2 {
    counters: Arc<ChainedTransformCounters>,
}

impl Plugin for ChainedTransformPlugin2 {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.plugin2_calls.fetch_add(1, Ordering::SeqCst);
                let mut payload = ctx.payload.clone();
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("plugin2_processed".into(), serde_json::json!(true));
                }
                JobScheduleResult::Continue(payload)
            }
        });
    }
}


#[derive(Clone)]
struct SkippingFirstPlugin;

impl Plugin for SkippingFirstPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(BeforeJobSchedule, move |ctx| async move {
            let should_skip = ctx
                .payload
                .get("skip_in_first")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if should_skip {
                return JobScheduleResult::Skip;
            }
            JobScheduleResult::Continue(ctx.payload)
        });
    }
}

#[derive(Clone)]
struct SecondPluginCounter {
    calls: Arc<AtomicU32>,
}

impl Plugin for SecondPluginCounter {
    fn register(self, hooks: &mut HookRegistry) {
        let calls = self.calls.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let calls = calls.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                JobScheduleResult::Continue(ctx.payload)
            }
        });
    }
}



#[derive(Debug, Default)]
struct ExtendedHookCounters {
    before_job_run: AtomicU32,
    after_job_run: AtomicU32,
    job_permanently_fail: AtomicU32,
}

#[derive(Clone)]
struct ExtendedHooksPlugin {
    counters: Arc<ExtendedHookCounters>,
}

impl ExtendedHooksPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(ExtendedHookCounters::default()),
        }
    }

    fn counters(&self) -> Arc<ExtendedHookCounters> {
        self.counters.clone()
    }
}

impl Plugin for ExtendedHooksPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobRun, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.before_job_run.fetch_add(1, Ordering::SeqCst);
                HookResult::Continue
            }
        });

        let counters = self.counters.clone();
        hooks.on(AfterJobRun, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.after_job_run.fetch_add(1, Ordering::SeqCst);
                HookResult::Continue
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobPermanentlyFail, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.job_permanently_fail.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}

#[derive(Clone)]
struct AfterJobRunResultPlugin {
    result: HookResult,
}

impl Plugin for AfterJobRunResultPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let result = self.result.clone();
        hooks.on(AfterJobRun, move |_ctx| {
            let result = result.clone();
            async move { result }
        });
    }
}




#[derive(Debug, Default)]
struct LocalQueueHookCounters {
    init: AtomicU32,
    set_mode: AtomicU32,
    get_jobs_complete: AtomicU32,
    return_jobs: AtomicU32,
    refetch_delay_start: AtomicU32,
    refetch_delay_expired: AtomicU32,
    last_mode_change: std::sync::Mutex<Option<(LocalQueueMode, LocalQueueMode)>>,
    last_jobs_count: AtomicUsize,
}

#[derive(Clone)]
struct LocalQueueHooksPlugin {
    counters: Arc<LocalQueueHookCounters>,
}

impl LocalQueueHooksPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(LocalQueueHookCounters::default()),
        }
    }

    fn counters(&self) -> Arc<LocalQueueHookCounters> {
        self.counters.clone()
    }
}

impl Plugin for LocalQueueHooksPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(LocalQueueInit, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.init.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(LocalQueueSetMode, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.set_mode.fetch_add(1, Ordering::SeqCst);
                *counters.last_mode_change.lock().unwrap() = Some((ctx.old_mode, ctx.new_mode));
            }
        });

        let counters = self.counters.clone();
        hooks.on(LocalQueueGetJobsComplete, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.get_jobs_complete.fetch_add(1, Ordering::SeqCst);
                counters
                    .last_jobs_count
                    .fetch_max(ctx.jobs_count, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(LocalQueueReturnJobs, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.return_jobs.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(LocalQueueRefetchDelayStart, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.refetch_delay_start.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(LocalQueueRefetchDelayExpired, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters
                    .refetch_delay_expired
                    .fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}

#[derive(Serialize, Deserialize)]
struct LocalQueueTestJob {
    id: u32,
}

impl TaskHandler for LocalQueueTestJob {
    const IDENTIFIER: &'static str = "local_queue_test_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}




#[derive(Serialize, Deserialize)]
struct SlowLocalQueueJob {
    id: u32,
}

impl TaskHandler for SlowLocalQueueJob {
    const IDENTIFIER: &'static str = "slow_local_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_secs(10)).await;
        Ok::<(), String>(())
    }
}
