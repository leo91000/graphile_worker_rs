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
