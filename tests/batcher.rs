use graphile_worker::{
    HookRegistry, IntoTaskHandlerResult, JobComplete, JobFail, JobPermanentlyFail, JobSpec, Plugin,
    TaskHandler, Worker, WorkerContext,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::spawn_local;
use tokio::time::{sleep, Instant};

use crate::helpers::with_test_db;

mod helpers;

#[derive(Clone, Debug)]
struct CompletedCounter(Arc<AtomicU32>);

impl CompletedCounter {
    fn new() -> Self {
        Self(Arc::new(AtomicU32::new(0)))
    }

    fn increment(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn get(&self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Default)]
struct BatcherHookCounters {
    complete: AtomicU32,
    fail: AtomicU32,
    permanent: AtomicU32,
}

#[derive(Clone, Debug)]
struct BatcherHooksPlugin {
    counters: Arc<BatcherHookCounters>,
}

impl BatcherHooksPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(BatcherHookCounters::default()),
        }
    }

    fn counters(&self) -> Arc<BatcherHookCounters> {
        self.counters.clone()
    }
}

impl Plugin for BatcherHooksPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(JobComplete, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.complete.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobFail, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.fail.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobPermanentlyFail, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.permanent.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}

#[derive(Serialize, Deserialize)]
struct SuccessJob {
    id: u32,
}

impl TaskHandler for SuccessJob {
    const IDENTIFIER: &'static str = "success_job";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        if let Some(counter) = ctx.get_ext::<CompletedCounter>() {
            counter.increment();
        }
        Ok::<(), String>(())
    }
}

#[derive(Serialize, Deserialize)]
struct FailJob {
    id: u32,
}

impl TaskHandler for FailJob {
    const IDENTIFIER: &'static str = "fail_job";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        if let Some(counter) = ctx.get_ext::<CompletedCounter>() {
            counter.increment();
        }
        Err::<(), String>(format!("Job {} failed", self.id))
    }
}

#[path = "batcher/completion.rs"]
mod completion;
#[path = "batcher/failure.rs"]
mod failure;
#[path = "batcher/hooks.rs"]
mod hooks;
#[path = "batcher/mixed.rs"]
mod mixed;
