use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker::{
    HookRegistry, IntoTaskHandlerResult, JobFail, JobInterrupted, JobSpec, Plugin, TaskHandler,
    Worker, WorkerContext,
};
use graphile_worker_runtime::sleep as runtime_sleep;
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Instant};

mod helpers;

use helpers::with_test_db;

#[derive(Debug, Default)]
struct RecoveryHookCounters {
    job_interrupted: AtomicU32,
    job_fail: AtomicU32,
}

#[derive(Clone)]
struct RecoveryHooksPlugin {
    counters: Arc<RecoveryHookCounters>,
}

impl RecoveryHooksPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(RecoveryHookCounters::default()),
        }
    }
}

impl Plugin for RecoveryHooksPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(JobInterrupted, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.job_interrupted.fetch_add(1, Ordering::SeqCst);
            }
        });

        let counters = self.counters.clone();
        hooks.on(JobFail, move |_ctx| {
            let counters = counters.clone();
            async move {
                counters.job_fail.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}

#[derive(Deserialize, Serialize)]
struct SlowJob {
    id: i64,
}

impl TaskHandler for SlowJob {
    const IDENTIFIER: &'static str = "slow_shutdown_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_secs(60)).await;
        Ok::<(), String>(())
    }
}

#[tokio::test]
async fn shutdown_aborted_job_is_returned_without_backoff() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(SlowJob { id: 1 }, JobSpec::default())
            .await
            .expect("failed to add job");

        let hooks_plugin = RecoveryHooksPlugin::new();
        let counters = hooks_plugin.counters.clone();
        let interrupted_job_retry_delay = Duration::from_secs(2);

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .shutdown_grace_period(Duration::from_millis(50))
                .shutdown_interrupted_job_retry_delay(interrupted_job_retry_delay)
                .listen_os_shutdown_signals(false)
                .define_job::<SlowJob>()
                .add_plugin(hooks_plugin)
                .init()
                .await
                .expect("failed to init worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = tokio::task::spawn(async move {
            let _ = worker_for_run.run().await;
        });

        let mut shutdown_requested_at = None;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.id == *job.id() && j.locked_by.is_some())
            {
                shutdown_requested_at = Some(chrono::Utc::now());
                worker.request_shutdown();
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        let shutdown_start = Instant::now();
        while !worker_fut.is_finished() {
            if shutdown_start.elapsed() > Duration::from_secs(10) {
                worker_fut.abort();
                panic!("worker should have shut down");
            }
            sleep(Duration::from_millis(50)).await;
        }

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should still exist");

        assert_eq!(recovered.attempts, 0, "attempts should be decremented back");
        assert!(recovered.locked_by.is_none(), "job should be unlocked");
        let expected_run_at = shutdown_requested_at.expect("shutdown should have been requested")
            + chrono::Duration::from_std(interrupted_job_retry_delay)
                .expect("interrupted job retry delay should convert to chrono duration")
            - chrono::Duration::milliseconds(500);
        assert!(
            recovered.run_at >= expected_run_at,
            "job should be rescheduled with recovery delay (expected >= {:?}, got {:?})",
            expected_run_at,
            recovered.run_at
        );
        assert_eq!(counters.job_interrupted.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_fail.load(Ordering::SeqCst), 0);
    })
    .await;
}
