use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use graphile_worker::{
    BeforeJobRunContext, HookResult, IntoTaskHandlerResult, JobCompleteContext, JobFailContext,
    JobSpec, JobStartContext, LifecycleHooks, TaskHandler, Worker, WorkerContext,
    WorkerShutdownContext, WorkerStartContext,
};
use serde::{Deserialize, Serialize};
use tokio::{
    task::spawn_local,
    time::{sleep, Duration, Instant},
};

use crate::helpers::with_test_db;

mod helpers;

#[derive(Debug, Default)]
struct HookCounters {
    worker_start: AtomicU32,
    worker_shutdown: AtomicU32,
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

impl LifecycleHooks for TestHooksPlugin {
    async fn on_worker_start(&self, _ctx: WorkerStartContext) {
        self.counters.worker_start.fetch_add(1, Ordering::SeqCst);
    }

    async fn on_worker_shutdown(&self, _ctx: WorkerShutdownContext) {
        self.counters.worker_shutdown.fetch_add(1, Ordering::SeqCst);
    }

    async fn on_job_start(&self, _ctx: JobStartContext) {
        self.counters.job_start.fetch_add(1, Ordering::SeqCst);
    }

    async fn on_job_complete(&self, _ctx: JobCompleteContext) {
        self.counters.job_complete.fetch_add(1, Ordering::SeqCst);
    }

    async fn on_job_fail(&self, _ctx: JobFailContext) {
        self.counters.job_fail.fetch_add(1, Ordering::SeqCst);
    }

    async fn before_job_run(&self, ctx: BeforeJobRunContext) -> HookResult {
        self.counters.before_job_run.fetch_add(1, Ordering::SeqCst);

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
            self.counters.skipped.fetch_add(1, Ordering::SeqCst);
            return HookResult::Skip;
        }

        if should_fail {
            self.counters.failed_by_hook.fetch_add(1, Ordering::SeqCst);
            return HookResult::Fail("Forced failure by hook".into());
        }

        HookResult::Continue
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

#[tokio::test]
async fn test_observer_hooks_are_called() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = TestHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        sleep(Duration::from_millis(100)).await;
        assert_eq!(counters.worker_start.load(Ordering::SeqCst), 1);

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: false,
                    force_fail: false,
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.job_complete.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have completed",
        )
        .await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_start.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_complete.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_fail.load(Ordering::SeqCst), 0);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_before_job_run_skip() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = TestHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: true,
                    force_fail: false,
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.skipped.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have been skipped",
        )
        .await;

        sleep(Duration::from_millis(200)).await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.skipped.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_start.load(Ordering::SeqCst), 0);
        assert_eq!(counters.job_complete.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_before_job_run_fail() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = TestHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: false,
                    force_fail: true,
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.failed_by_hook.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have been failed by hook",
        )
        .await;

        sleep(Duration::from_millis(200)).await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.failed_by_hook.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_start.load(Ordering::SeqCst), 0);
        assert_eq!(counters.job_fail.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_job_fail_hook_on_task_error() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = TestHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: false,
                    force_fail: false,
                    should_error: true,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.job_fail.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have failed",
        )
        .await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_start.load(Ordering::SeqCst), 1);
        assert_eq!(counters.job_complete.load(Ordering::SeqCst), 0);
        assert_eq!(counters.job_fail.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_multiple_plugins() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin1 = TestHooksPlugin::new();
        let plugin2 = TestHooksPlugin::new();
        let counters1 = plugin1.counters();
        let counters2 = plugin2.counters();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .define_job::<TestJob>()
                    .add_plugin(plugin1)
                    .add_plugin(plugin2)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        utils
            .add_job(
                TestJob {
                    value: 1,
                    skip: false,
                    force_fail: false,
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c1 = counters1.clone();
        let c2 = counters2.clone();
        wait_for_condition(
            || {
                c1.job_complete.load(Ordering::SeqCst) >= 1
                    && c2.job_complete.load(Ordering::SeqCst) >= 1
            },
            5,
            "Both plugins should have seen job complete",
        )
        .await;

        assert_eq!(counters1.job_start.load(Ordering::SeqCst), 1);
        assert_eq!(counters2.job_start.load(Ordering::SeqCst), 1);
        assert_eq!(counters1.job_complete.load(Ordering::SeqCst), 1);
        assert_eq!(counters2.job_complete.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}
