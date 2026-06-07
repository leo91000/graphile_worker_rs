use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use graphile_worker::{
    HookRegistry, JobComplete, JobFail, JobStart, Plugin, WorkerShutdown, WorkerStart,
};

#[derive(Debug)]
pub(super) struct MetricsPlugin {
    jobs_started: AtomicU64,
    jobs_completed: AtomicU64,
    jobs_failed: AtomicU64,
}

impl MetricsPlugin {
    pub(super) fn new() -> Self {
        Self {
            jobs_started: AtomicU64::new(0),
            jobs_completed: AtomicU64::new(0),
            jobs_failed: AtomicU64::new(0),
        }
    }
}

impl Plugin for MetricsPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(WorkerStart, async |ctx| {
            println!("[MetricsPlugin] Worker {} started", ctx.worker_id);
        });

        let jobs_started = Arc::new(self.jobs_started);
        let jobs_completed = Arc::new(self.jobs_completed);
        let jobs_failed = Arc::new(self.jobs_failed);

        {
            let jobs_started = jobs_started.clone();
            let jobs_completed = jobs_completed.clone();
            let jobs_failed = jobs_failed.clone();
            hooks.on(WorkerShutdown, move |ctx| {
                let jobs_started = jobs_started.clone();
                let jobs_completed = jobs_completed.clone();
                let jobs_failed = jobs_failed.clone();
                async move {
                    println!(
                        "[MetricsPlugin] Worker {} shutting down (reason: {:?})",
                        ctx.worker_id, ctx.reason
                    );
                    println!(
                        "Stats: started={}, completed={}, failed={}",
                        jobs_started.load(Ordering::Relaxed),
                        jobs_completed.load(Ordering::Relaxed),
                        jobs_failed.load(Ordering::Relaxed)
                    );
                }
            });
        }

        {
            let jobs_started = jobs_started.clone();
            hooks.on(JobStart, move |ctx| {
                let jobs_started = jobs_started.clone();
                async move {
                    jobs_started.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} started (task: {})",
                        ctx.job.id(),
                        ctx.job.task_identifier()
                    );
                }
            });
        }

        {
            let jobs_completed = jobs_completed.clone();
            hooks.on(JobComplete, move |ctx| {
                let jobs_completed = jobs_completed.clone();
                async move {
                    jobs_completed.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} completed in {:?}",
                        ctx.job.id(),
                        ctx.duration
                    );
                }
            });
        }

        {
            let jobs_failed = jobs_failed.clone();
            hooks.on(JobFail, move |ctx| {
                let jobs_failed = jobs_failed.clone();
                async move {
                    jobs_failed.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "[MetricsPlugin] Job {} failed: {} (will_retry: {})",
                        ctx.job.id(),
                        ctx.error,
                        ctx.will_retry
                    );
                }
            });
        }
    }
}
