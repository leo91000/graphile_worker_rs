use graphile_worker::{
    IntoTaskHandlerResult, JobCompleted, JobFailed, JobLifecycleHooks, JobSpec, JobStarted,
    LifeCycleEvent, TaskHandler, Worker, WorkerContext,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    task::spawn_local,
    time::{sleep, Duration, Instant},
};

use crate::helpers::{with_test_db, StaticCounter};

mod helpers;

#[derive(Debug, Default, Clone)]
struct HookEvents {
    started: Vec<JobStarted>,
    completed: Vec<JobCompleted>,
    failed: Vec<JobFailed>,
}

struct TestHooks {
    events: Arc<Mutex<HookEvents>>,
}

impl TestHooks {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(HookEvents::default())),
        }
    }
}

impl JobLifecycleHooks for TestHooks {
    fn on_event(&self, event: LifeCycleEvent) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let events = self.events.clone();
        Box::pin(async move {
            match event {
                LifeCycleEvent::Started(started) => {
                    events.lock().await.started.push(started);
                }
                LifeCycleEvent::Completed(completed) => {
                    events.lock().await.completed.push(completed);
                }
                LifeCycleEvent::Failed(failed) => {
                    events.lock().await.failed.push(failed);
                }
            }
        })
    }
}

#[tokio::test]
async fn hooks_fire_for_successful_jobs() {
    static SUCCESS_JOB_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct SuccessJob {
        value: u32,
    }

    impl TaskHandler for SuccessJob {
        const IDENTIFIER: &'static str = "success_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            SUCCESS_JOB_CALL_COUNT.increment().await;
            Ok::<(), String>(())
        }
    }

    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let hooks = TestHooks::new();
        let hooks_events = hooks.events.clone();

        // Create a worker with hooks
        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(1)
                    .define_job::<SuccessJob>()
                    .with_hooks(hooks)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        // Add a job
        utils
            .add_job(SuccessJob { value: 42 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        // Wait for job to complete
        let start_time = Instant::now();
        while SUCCESS_JOB_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job should have been executed by now");
            }
            sleep(Duration::from_millis(100)).await;
        }

        // Give hooks time to fire
        sleep(Duration::from_millis(500)).await;

        // Check hook events
        let events = hooks_events.lock().await;
        assert_eq!(events.started.len(), 1, "Should have one started event");
        assert_eq!(events.completed.len(), 1, "Should have one completed event");
        assert_eq!(events.failed.len(), 0, "Should have no failed events");

        let started = &events.started[0];
        assert_eq!(started.task_identifier, "success_job");
        assert_eq!(started.attempts, 1);

        let completed = &events.completed[0];
        assert_eq!(completed.task_identifier, "success_job");
        assert_eq!(completed.attempts, 1);
        // Duration is captured and non-negative by definition

        // Abort the worker
        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn hooks_fire_for_failed_jobs() {
    static FAIL_JOB_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct FailJob {
        value: u32,
    }

    impl TaskHandler for FailJob {
        const IDENTIFIER: &'static str = "fail_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            FAIL_JOB_CALL_COUNT.increment().await;
            Err::<(), String>("Intentional failure".to_string())
        }
    }

    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let hooks = TestHooks::new();
        let hooks_events = hooks.events.clone();

        // Create a worker with hooks
        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(1)
                    .define_job::<FailJob>()
                    .with_hooks(hooks)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        // Add a job with max_attempts = 1 so it doesn't retry
        utils
            .add_job(
                FailJob { value: 42 },
                JobSpec::builder().max_attempts(1).build(),
            )
            .await
            .expect("Failed to add job");

        // Wait for job to fail
        let start_time = Instant::now();
        while FAIL_JOB_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job should have been executed by now");
            }
            sleep(Duration::from_millis(100)).await;
        }

        // Give hooks time to fire
        sleep(Duration::from_millis(500)).await;

        // Check hook events
        let events = hooks_events.lock().await;
        assert_eq!(events.started.len(), 1, "Should have one started event");
        assert_eq!(events.completed.len(), 0, "Should have no completed events");
        assert_eq!(events.failed.len(), 1, "Should have one failed event");

        let started = &events.started[0];
        assert_eq!(started.task_identifier, "fail_job");
        assert_eq!(started.attempts, 1);

        let failed = &events.failed[0];
        assert_eq!(failed.task_identifier, "fail_job");
        assert_eq!(failed.attempts, 1);
        assert!(!failed.will_retry, "Job should not retry (max_attempts=1)");
        assert!(failed.error.contains("Intentional failure"));
        // Duration is captured and non-negative by definition

        // Abort the worker
        worker_fut.abort();
    })
    .await;
}

#[tokio::test]
async fn hooks_fire_for_retried_jobs() {
    static RETRY_JOB_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct RetryJob {
        value: u32,
    }

    impl TaskHandler for RetryJob {
        const IDENTIFIER: &'static str = "retry_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            let count = RETRY_JOB_CALL_COUNT.increment().await;
            if count < 2 {
                Err::<(), String>("First attempt fails".to_string())
            } else {
                Ok::<(), String>(())
            }
        }
    }

    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let hooks = TestHooks::new();
        let hooks_events = hooks.events.clone();

        // Create a worker with hooks
        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(1)
                    .define_job::<RetryJob>()
                    .with_hooks(hooks)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        // Add a job that will retry
        utils
            .add_job(RetryJob { value: 42 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        // Wait for job to succeed on retry
        let start_time = Instant::now();
        while RETRY_JOB_CALL_COUNT.get().await < 2 {
            if start_time.elapsed().as_secs() > 10 {
                panic!("Job should have been executed twice by now");
            }
            sleep(Duration::from_millis(100)).await;
        }

        // Give hooks time to fire
        sleep(Duration::from_millis(500)).await;

        // Check hook events
        let events = hooks_events.lock().await;
        assert_eq!(
            events.started.len(),
            2,
            "Should have two started events (initial + retry)"
        );
        assert_eq!(events.completed.len(), 1, "Should have one completed event");
        assert_eq!(events.failed.len(), 1, "Should have one failed event");

        // First attempt
        let first_started = &events.started[0];
        assert_eq!(first_started.attempts, 1);

        let first_failed = &events.failed[0];
        assert_eq!(first_failed.attempts, 1);
        assert!(first_failed.will_retry, "Job should retry");

        // Second attempt (retry)
        let second_started = &events.started[1];
        assert_eq!(second_started.attempts, 2);

        let completed = &events.completed[0];
        assert_eq!(completed.attempts, 2);

        // Abort the worker
        worker_fut.abort();
    })
    .await;
}
