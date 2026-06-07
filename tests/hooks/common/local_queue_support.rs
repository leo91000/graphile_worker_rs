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
