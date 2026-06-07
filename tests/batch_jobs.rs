use graphile_worker::{
    BatchTaskHandler, BatchTaskResult, BeforeJobSchedule, HookRegistry, IntoBatchTaskHandlerResult,
    IntoTaskHandlerResult, JobFail, JobPermanentlyFail, JobScheduleResult, JobSpec, Plugin,
    TaskHandler, WorkerContext, WorkerContextExt,
};
use helpers::{with_test_db, StaticCounter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

mod helpers;

#[derive(Clone, Default)]
struct BatchFailureHookPlugin {
    fail_count: Arc<AtomicU32>,
    permanent_count: Arc<AtomicU32>,
}

impl BatchFailureHookPlugin {
    fn fail_count(&self) -> u32 {
        self.fail_count.load(Ordering::SeqCst)
    }

    fn permanent_count(&self) -> u32 {
        self.permanent_count.load(Ordering::SeqCst)
    }
}

impl Plugin for BatchFailureHookPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let fail_count = self.fail_count.clone();
        hooks.on(JobFail, move |_ctx| {
            let fail_count = fail_count.clone();
            async move {
                fail_count.fetch_add(1, Ordering::SeqCst);
            }
        });

        let permanent_count = self.permanent_count.clone();
        hooks.on(JobPermanentlyFail, move |_ctx| {
            let permanent_count = permanent_count.clone();
            async move {
                permanent_count.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}

struct BatchPayloadOverridePlugin {
    payload: Value,
}

impl Plugin for BatchPayloadOverridePlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let payload = self.payload;
        hooks.on(BeforeJobSchedule, move |_ctx| {
            let payload = payload.clone();
            async move { JobScheduleResult::Continue(payload) }
        });
    }
}

#[path = "batch_jobs/failures.rs"]
mod failures;
#[path = "batch_jobs/hook_rewrites.rs"]
mod hook_rewrites;
#[path = "batch_jobs/success.rs"]
mod success;
