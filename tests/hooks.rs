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
use serde::{Deserialize, Serialize};
use tokio::{
    task::spawn_local,
    time::{sleep, Duration, Instant},
};

use crate::helpers::with_test_db;

mod helpers;

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

        let c = counters.clone();
        wait_for_condition(
            || c.worker_start.load(Ordering::SeqCst) >= 1,
            5,
            "Worker should have started",
        )
        .await;
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

        assert_eq!(counters.job_fetch.load(Ordering::SeqCst), 1);
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

#[tokio::test]
async fn test_before_job_schedule_transform() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();
        let run_plugin = TestHooksPlugin::new();
        let run_counters = run_plugin.counters();

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(schedule_plugin)
            .add_plugin(run_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let worker_fut = spawn_local(async move {
            worker.run().await.expect("Failed to run worker");
        });

        sleep(Duration::from_millis(100)).await;

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "transform": true
                }),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = run_counters.clone();
        wait_for_condition(
            || c.job_complete.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have completed",
        )
        .await;

        assert_eq!(schedule_counters.before_schedule.load(Ordering::SeqCst), 1);
        assert_eq!(schedule_counters.transformed.load(Ordering::SeqCst), 1);
        assert_eq!(run_counters.job_complete.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_before_job_schedule_skip() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(schedule_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let worker_fut = spawn_local(async move {
            worker.run().await.expect("Failed to run worker");
        });

        sleep(Duration::from_millis(100)).await;

        let result = worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "skip_schedule": true
                }),
                JobSpec::default(),
            )
            .await;

        assert!(result.is_err());
        assert_eq!(schedule_counters.before_schedule.load(Ordering::SeqCst), 1);
        assert_eq!(schedule_counters.skipped.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_before_job_schedule_fail() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(schedule_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let worker_fut = spawn_local(async move {
            worker.run().await.expect("Failed to run worker");
        });

        sleep(Duration::from_millis(100)).await;

        let result = worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "fail_schedule": true
                }),
                JobSpec::default(),
            )
            .await;

        assert!(result.is_err());
        assert_eq!(schedule_counters.before_schedule.load(Ordering::SeqCst), 1);
        assert_eq!(schedule_counters.failed.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_before_job_schedule_transform_payload_stored_in_db() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(schedule_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 42,
                    "transform": true
                }),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        assert_eq!(schedule_counters.transformed.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);

        let job_payload = &jobs[0].payload;
        assert_eq!(job_payload.get("value").unwrap().as_u64().unwrap(), 42);
        assert!(job_payload.get("transform").unwrap().as_bool().unwrap());
        assert!(
            job_payload.get("transformed").unwrap().as_bool().unwrap(),
            "Transformed field should be added by hook"
        );
    })
    .await;
}

#[tokio::test]
async fn test_before_job_schedule_skip_no_job_in_db() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(schedule_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let result = worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "skip_schedule": true
                }),
                JobSpec::default(),
            )
            .await;

        assert!(result.is_err());
        assert_eq!(schedule_counters.skipped.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0, "Skipped job should not be in database");
    })
    .await;
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

#[tokio::test]
async fn test_before_job_schedule_receives_correct_context() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = IdentifierCapturingPlugin::new();
        let counters = plugin.counters();

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({ "value": 1 }),
                JobSpec::builder()
                    .queue_name("test_queue")
                    .priority(10)
                    .build(),
            )
            .await
            .expect("Failed to add job");

        let captured_id = counters.captured_identifier.lock().unwrap().clone();
        assert_eq!(captured_id, Some("test_hooks_job".to_string()));

        let captured_queue = counters.captured_spec_queue.lock().unwrap().clone();
        assert_eq!(captured_queue, Some("test_queue".to_string()));

        let captured_priority = *counters.captured_spec_priority.lock().unwrap();
        assert_eq!(captured_priority, Some(10));
    })
    .await;
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

#[tokio::test]
async fn test_before_job_schedule_with_typed_add_job() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TypedScheduleJob>()
            .add_plugin(schedule_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        worker_utils
            .add_job(
                TypedScheduleJob {
                    message: "hello".to_string(),
                    transform: true,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        assert_eq!(schedule_counters.before_schedule.load(Ordering::SeqCst), 1);
        assert_eq!(schedule_counters.transformed.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task_identifier, "typed_schedule_job");

        let payload = &jobs[0].payload;
        assert_eq!(payload.get("message").unwrap().as_str().unwrap(), "hello");
        assert!(
            payload.get("transformed").unwrap().as_bool().unwrap(),
            "Hook should have added transformed field"
        );
    })
    .await;
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

#[tokio::test]
async fn test_before_job_schedule_multiple_plugins_chain_transforms() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counters = Arc::new(ChainedTransformCounters::default());
        let plugin1 = ChainedTransformPlugin1 {
            counters: counters.clone(),
        };
        let plugin2 = ChainedTransformPlugin2 {
            counters: counters.clone(),
        };

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(plugin1)
            .add_plugin(plugin2)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({ "value": 1 }),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        assert_eq!(counters.plugin1_calls.load(Ordering::SeqCst), 1);
        assert_eq!(counters.plugin2_calls.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);

        let payload = &jobs[0].payload;
        assert!(
            payload.get("plugin1_processed").unwrap().as_bool().unwrap(),
            "Plugin 1 should have processed"
        );
        assert!(
            payload.get("plugin2_processed").unwrap().as_bool().unwrap(),
            "Plugin 2 should have processed"
        );
    })
    .await;
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

#[tokio::test]
async fn test_before_job_schedule_skip_stops_chain() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let second_plugin_calls = Arc::new(AtomicU32::new(0));
        let second_plugin = SecondPluginCounter {
            calls: second_plugin_calls.clone(),
        };

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(SkippingFirstPlugin)
            .add_plugin(second_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let result = worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "skip_in_first": true
                }),
                JobSpec::default(),
            )
            .await;

        assert!(result.is_err());
        assert_eq!(
            second_plugin_calls.load(Ordering::SeqCst),
            0,
            "Second plugin should not be called when first plugin skips"
        );

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 0);
    })
    .await;
}

#[tokio::test]
async fn test_before_job_schedule_and_before_job_run_both_called() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let schedule_plugin = ScheduleHooksPlugin::new();
        let schedule_counters = schedule_plugin.counters();
        let run_plugin = TestHooksPlugin::new();
        let run_counters = run_plugin.counters();

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(2)
            .define_job::<TestJob>()
            .add_plugin(schedule_plugin)
            .add_plugin(run_plugin)
            .init()
            .await
            .expect("Failed to create worker");

        let worker_utils = worker.create_utils();

        let worker_fut = spawn_local(async move {
            worker.run().await.expect("Failed to run worker");
        });

        sleep(Duration::from_millis(100)).await;

        worker_utils
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({ "value": 1 }),
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = run_counters.clone();
        wait_for_condition(
            || c.job_complete.load(Ordering::SeqCst) >= 1,
            5,
            "Job should have completed",
        )
        .await;

        assert_eq!(
            schedule_counters.before_schedule.load(Ordering::SeqCst),
            1,
            "Schedule hook should be called"
        );
        assert_eq!(
            run_counters.before_job_run.load(Ordering::SeqCst),
            1,
            "Run hook should be called"
        );
        assert_eq!(
            run_counters.job_start.load(Ordering::SeqCst),
            1,
            "Job should have started"
        );
        assert_eq!(
            run_counters.job_complete.load(Ordering::SeqCst),
            1,
            "Job should have completed"
        );

        worker_fut.abort();
    })
    .await;
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

#[tokio::test]
async fn test_after_job_run_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = ExtendedHooksPlugin::new();
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
                    should_error: false,
                },
                JobSpec::default(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.after_job_run.load(Ordering::SeqCst) >= 1,
            5,
            "after_job_run should have been called",
        )
        .await;

        assert_eq!(counters.before_job_run.load(Ordering::SeqCst), 1);
        assert_eq!(counters.after_job_run.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_on_job_permanently_fail_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = ExtendedHooksPlugin::new();
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
            .add_raw_job(
                "test_hooks_job",
                serde_json::json!({
                    "value": 1,
                    "should_error": true
                }),
                JobSpec::builder().max_attempts(1).build(),
            )
            .await
            .expect("Failed to add job");

        let c = counters.clone();
        wait_for_condition(
            || c.job_permanently_fail.load(Ordering::SeqCst) >= 1,
            5,
            "on_job_permanently_fail should have been called",
        )
        .await;

        assert_eq!(counters.job_permanently_fail.load(Ordering::SeqCst), 1);

        worker_fut.abort();
    })
    .await;
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
                    .store(ctx.jobs_count, Ordering::SeqCst);
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

#[tokio::test]
async fn test_local_queue_init_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .local_queue(LocalQueueConfig::default().with_size(10))
                    .define_job::<LocalQueueTestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        sleep(Duration::from_millis(200)).await;

        assert_eq!(
            counters.init.load(Ordering::SeqCst),
            1,
            "LocalQueue init hook should be called once"
        );

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_local_queue_set_mode_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .local_queue(LocalQueueConfig::default().with_size(10))
                    .define_job::<LocalQueueTestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        sleep(Duration::from_millis(200)).await;

        assert!(
            counters.set_mode.load(Ordering::SeqCst) >= 1,
            "LocalQueue set_mode hook should be called at least once (starting -> polling)"
        );

        let last_mode = *counters.last_mode_change.lock().unwrap();
        assert!(last_mode.is_some(), "Should have recorded a mode change");

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_local_queue_get_jobs_complete_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        for i in 1..=5 {
            utils
                .add_job(LocalQueueTestJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .local_queue(LocalQueueConfig::default().with_size(10))
                    .define_job::<LocalQueueTestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let c = counters.clone();
        wait_for_condition(
            || c.last_jobs_count.load(Ordering::SeqCst) >= 1,
            5,
            "Should have fetched at least one job",
        )
        .await;

        assert!(
            counters.get_jobs_complete.load(Ordering::SeqCst) >= 1,
            "get_jobs_complete hook should have been called"
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct SlowLocalQueueJob {
    id: u32,
}

impl TaskHandler for SlowLocalQueueJob {
    const IDENTIFIER: &'static str = "slow_local_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        sleep(Duration::from_secs(10)).await;
        Ok::<(), String>(())
    }
}

#[tokio::test]
async fn test_local_queue_return_jobs_hook() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        for i in 1..=10 {
            utils
                .add_job(SlowLocalQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(2)
                .local_queue(LocalQueueConfig::default().with_size(20))
                .listen_os_shutdown_signals(false)
                .define_job::<SlowLocalQueueJob>()
                .add_plugin(plugin)
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(300)).await;

        worker.request_shutdown();

        let c = counters.clone();
        wait_for_condition(
            || c.return_jobs.load(Ordering::SeqCst) >= 1,
            10,
            "return_jobs hook should be called on shutdown",
        )
        .await;

        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn test_local_queue_refetch_delay_hooks() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let plugin = LocalQueueHooksPlugin::new();
        let counters = plugin.counters();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .local_queue(
                        LocalQueueConfig::default()
                            .with_size(10)
                            .with_refetch_delay(
                                RefetchDelayConfig::default()
                                    .with_duration(Duration::from_millis(50))
                                    .with_threshold(0),
                            ),
                    )
                    .define_job::<LocalQueueTestJob>()
                    .add_plugin(plugin)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        sleep(Duration::from_millis(500)).await;

        assert!(
            counters.refetch_delay_start.load(Ordering::SeqCst) >= 1,
            "refetch_delay_start hook should be called"
        );

        let c = counters.clone();
        wait_for_condition(
            || c.refetch_delay_expired.load(Ordering::SeqCst) >= 1,
            5,
            "refetch_delay_expired hook should be called",
        )
        .await;

        worker_fut.abort();
    })
    .await;
}
