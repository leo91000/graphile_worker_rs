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
