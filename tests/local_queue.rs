use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, LocalQueueConfig, RefetchDelayConfig, TaskHandler, Worker,
    WorkerContext,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    task::spawn_local,
    time::{sleep, Instant},
};

use crate::helpers::{with_test_db, StaticCounter};

mod helpers;

#[derive(Serialize, Deserialize)]
struct LocalQueueJob {
    id: u32,
}

static LOCAL_QUEUE_JOB_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for LocalQueueJob {
    const IDENTIFIER: &'static str = "local_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        LOCAL_QUEUE_JOB_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_processes_jobs_correctly() {
    with_test_db(|test_db| async move {
        LOCAL_QUEUE_JOB_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(3)
                    .local_queue(LocalQueueConfig::default().with_size(10))
                    .define_job::<LocalQueueJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        for i in 1..=5 {
            utils
                .add_job(LocalQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");

            let start_time = Instant::now();
            while LOCAL_QUEUE_JOB_CALL_COUNT.get().await < i {
                if start_time.elapsed().as_secs() > 5 {
                    panic!("Job should have been executed by now");
                }
                sleep(Duration::from_millis(100)).await;
            }

            assert_eq!(
                LOCAL_QUEUE_JOB_CALL_COUNT.get().await,
                i,
                "Job should have been executed {} times",
                i
            );
        }

        sleep(Duration::from_secs(1)).await;
        assert_eq!(
            LOCAL_QUEUE_JOB_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct BatchJob {
    id: u32,
}

static BATCH_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for BatchJob {
    const IDENTIFIER: &'static str = "batch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        BATCH_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_batch_fetches_jobs() {
    with_test_db(|test_db| async move {
        BATCH_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=20 {
            utils
                .add_job(BatchJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(5)
                    .local_queue(LocalQueueConfig::default().with_size(50))
                    .define_job::<BatchJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while BATCH_CALL_COUNT.get().await < 20 {
            if start_time.elapsed().as_secs() > 10 {
                panic!(
                    "All jobs should have been executed by now, got {}",
                    BATCH_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            BATCH_CALL_COUNT.get().await,
            20,
            "All 20 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct ShutdownJob {
    id: u32,
}

static SHUTDOWN_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ShutdownJob {
    const IDENTIFIER: &'static str = "shutdown_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        sleep(Duration::from_secs(10)).await;
        SHUTDOWN_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_returns_jobs_on_shutdown() {
    with_test_db(|test_db| async move {
        SHUTDOWN_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=10 {
            utils
                .add_job(ShutdownJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let initial_jobs = test_db.get_jobs().await;
        assert_eq!(initial_jobs.len(), 10, "Should have 10 jobs initially");

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(2)
                .local_queue(LocalQueueConfig::default().with_size(20))
                .listen_os_shutdown_signals(false)
                .define_job::<ShutdownJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(500)).await;

        worker.request_shutdown();

        let start_time = Instant::now();
        while !worker_fut.is_finished() {
            if start_time.elapsed().as_secs() > 10 {
                worker_fut.abort();
                panic!("Worker should have shut down by now");
            }
            sleep(Duration::from_millis(100)).await;
        }

        sleep(Duration::from_millis(200)).await;

        let remaining_jobs = test_db.get_jobs().await;
        let unlocked_jobs: Vec<_> = remaining_jobs
            .iter()
            .filter(|j| j.locked_by.is_none())
            .collect();

        assert!(
            unlocked_jobs.len() >= 8,
            "Most jobs should be returned to the queue (got {} unlocked out of {})",
            unlocked_jobs.len(),
            remaining_jobs.len()
        );

        assert_eq!(
            SHUTDOWN_CALL_COUNT.get().await,
            0,
            "No jobs should have completed (they take 10s each)"
        );
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct FlaggedJob {
    id: u32,
}

static FLAGGED_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for FlaggedJob {
    const IDENTIFIER: &'static str = "flagged_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        FLAGGED_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_with_forbidden_flags_uses_direct_fetch() {
    with_test_db(|test_db| async move {
        FLAGGED_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=3 {
            utils
                .add_job(FlaggedJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        for i in 4..=6 {
            utils
                .add_job(
                    FlaggedJob { id: i },
                    JobSpec {
                        flags: Some(vec!["special".to_string()]),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job with flag");
        }

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(3)
                    .local_queue(LocalQueueConfig::default().with_size(10))
                    .add_forbidden_flag("special")
                    .define_job::<FlaggedJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while FLAGGED_CALL_COUNT.get().await < 3 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Jobs without flag should have been executed by now");
            }
            sleep(Duration::from_millis(100)).await;
        }

        sleep(Duration::from_millis(500)).await;

        assert_eq!(
            FLAGGED_CALL_COUNT.get().await,
            3,
            "Only 3 jobs without the special flag should have been executed"
        );

        let remaining_jobs = test_db.get_jobs().await;
        assert_eq!(
            remaining_jobs.len(),
            3,
            "3 jobs with the special flag should remain in the queue"
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct RunOnceJob {
    id: u32,
}

static RUN_ONCE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RunOnceJob {
    const IDENTIFIER: &'static str = "run_once_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        RUN_ONCE_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_works_with_run_once() {
    with_test_db(|test_db| async move {
        RUN_ONCE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=5 {
            utils
                .add_job(RunOnceJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker = Worker::options()
            .pg_pool(test_db.test_pool.clone())
            .concurrency(3)
            .define_job::<RunOnceJob>()
            .init()
            .await
            .expect("Failed to create worker");

        worker.run_once().await.expect("Failed to run_once");

        assert_eq!(
            RUN_ONCE_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed with run_once"
        );

        let remaining_jobs = test_db.get_jobs().await;
        assert_eq!(
            remaining_jobs.len(),
            0,
            "No jobs should remain after run_once"
        );
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct TtlExpiryJob {
    id: u32,
}

static TTL_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for TtlExpiryJob {
    const IDENTIFIER: &'static str = "ttl_expiry_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        sleep(Duration::from_secs(30)).await;
        TTL_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_returns_jobs_on_ttl_expiry() {
    with_test_db(|test_db| async move {
        TTL_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=10 {
            utils
                .add_job(TtlExpiryJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let initial_jobs = test_db.get_jobs().await;
        assert_eq!(initial_jobs.len(), 10, "Should have 10 jobs initially");

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(1)
                .local_queue(
                    LocalQueueConfig::default()
                        .with_size(20)
                        .with_ttl(Duration::from_millis(500)),
                )
                .listen_os_shutdown_signals(false)
                .define_job::<TtlExpiryJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(200)).await;

        let jobs_during_processing = test_db.get_jobs().await;
        let locked_jobs: Vec<_> = jobs_during_processing
            .iter()
            .filter(|j| j.locked_by.is_some())
            .collect();
        assert!(
            locked_jobs.len() >= 1,
            "At least one job should be locked by worker"
        );

        sleep(Duration::from_millis(800)).await;

        let jobs_after_ttl = test_db.get_jobs().await;
        let unlocked_jobs: Vec<_> = jobs_after_ttl
            .iter()
            .filter(|j| j.locked_by.is_none())
            .collect();

        assert!(
            unlocked_jobs.len() >= 8,
            "Most jobs should be returned after TTL expiry (got {} unlocked out of {})",
            unlocked_jobs.len(),
            jobs_after_ttl.len()
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct RefetchDelayJob {
    id: u32,
}

static REFETCH_DELAY_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RefetchDelayJob {
    const IDENTIFIER: &'static str = "refetch_delay_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        REFETCH_DELAY_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_refetch_delay_triggers_when_below_threshold() {
    with_test_db(|test_db| async move {
        REFETCH_DELAY_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=3 {
            utils
                .add_job(RefetchDelayJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .poll_interval(Duration::from_secs(10))
                    .local_queue(
                        LocalQueueConfig::default()
                            .with_size(10)
                            .with_refetch_delay(
                                RefetchDelayConfig::default()
                                    .with_duration(Duration::from_millis(200))
                                    .with_threshold(5),
                            ),
                    )
                    .define_job::<RefetchDelayJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while REFETCH_DELAY_CALL_COUNT.get().await < 3 {
            if start_time.elapsed().as_secs() > 5 {
                panic!(
                    "Jobs should have been executed by now, got {}",
                    REFETCH_DELAY_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(
            REFETCH_DELAY_CALL_COUNT.get().await,
            3,
            "All 3 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct SmallQueueJob {
    id: u32,
}

static SMALL_QUEUE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for SmallQueueJob {
    const IDENTIFIER: &'static str = "small_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        SMALL_QUEUE_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_with_size_one() {
    with_test_db(|test_db| async move {
        SMALL_QUEUE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=5 {
            utils
                .add_job(SmallQueueJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(1)
                    .local_queue(LocalQueueConfig::default().with_size(1))
                    .define_job::<SmallQueueJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while SMALL_QUEUE_CALL_COUNT.get().await < 5 {
            if start_time.elapsed().as_secs() > 10 {
                panic!(
                    "All jobs should have been executed by now, got {}",
                    SMALL_QUEUE_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            SMALL_QUEUE_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed even with queue size 1"
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct ConcurrentDistributionJob {
    id: u32,
}

static CONCURRENT_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ConcurrentDistributionJob {
    const IDENTIFIER: &'static str = "concurrent_distribution_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        sleep(Duration::from_millis(100)).await;
        CONCURRENT_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_distributes_jobs_to_concurrent_workers() {
    with_test_db(|test_db| async move {
        CONCURRENT_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=10 {
            utils
                .add_job(ConcurrentDistributionJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(5)
                    .local_queue(LocalQueueConfig::default().with_size(20))
                    .define_job::<ConcurrentDistributionJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        while CONCURRENT_CALL_COUNT.get().await < 10 {
            if start.elapsed().as_secs() > 10 {
                panic!(
                    "All jobs should have been executed by now, got {}",
                    CONCURRENT_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(3),
            "With concurrency 5, 10 jobs at 100ms each should complete faster than sequential (10s), took {:?}",
            elapsed
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct ModeTransitionJob {
    id: u32,
}

static MODE_TRANSITION_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for ModeTransitionJob {
    const IDENTIFIER: &'static str = "mode_transition_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        sleep(Duration::from_millis(50)).await;
        MODE_TRANSITION_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_transitions_modes_correctly() {
    with_test_db(|test_db| async move {
        MODE_TRANSITION_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(1)
                .poll_interval(Duration::from_millis(100))
                .local_queue(LocalQueueConfig::default().with_size(5))
                .listen_os_shutdown_signals(false)
                .define_job::<ModeTransitionJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(50)).await;

        for i in 1..=3 {
            utils
                .add_job(ModeTransitionJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start_time = Instant::now();
        while MODE_TRANSITION_CALL_COUNT.get().await < 3 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("First batch should have been executed");
            }
            sleep(Duration::from_millis(50)).await;
        }

        sleep(Duration::from_millis(200)).await;

        for i in 4..=6 {
            utils
                .add_job(ModeTransitionJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start_time = Instant::now();
        while MODE_TRANSITION_CALL_COUNT.get().await < 6 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Second batch should have been executed");
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(
            MODE_TRANSITION_CALL_COUNT.get().await,
            6,
            "All 6 jobs should have been executed across mode transitions"
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct EmptyQueueJob {
    id: u32,
}

static EMPTY_QUEUE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for EmptyQueueJob {
    const IDENTIFIER: &'static str = "empty_queue_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        EMPTY_QUEUE_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_handles_empty_queue_gracefully() {
    with_test_db(|test_db| async move {
        EMPTY_QUEUE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(2)
                .poll_interval(Duration::from_millis(100))
                .local_queue(LocalQueueConfig::default().with_size(10))
                .listen_os_shutdown_signals(false)
                .define_job::<EmptyQueueJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(300)).await;

        assert_eq!(
            EMPTY_QUEUE_CALL_COUNT.get().await,
            0,
            "No jobs should have been executed on empty queue"
        );

        utils
            .add_job(EmptyQueueJob { id: 1 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        let start_time = Instant::now();
        while EMPTY_QUEUE_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job should have been executed");
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(
            EMPTY_QUEUE_CALL_COUNT.get().await,
            1,
            "Job should have been executed after being added to empty queue"
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct LargeBatchJob {
    id: u32,
}

static LARGE_BATCH_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for LargeBatchJob {
    const IDENTIFIER: &'static str = "large_batch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        LARGE_BATCH_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_handles_large_batch() {
    with_test_db(|test_db| async move {
        LARGE_BATCH_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=100 {
            utils
                .add_job(LargeBatchJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(10)
                    .local_queue(LocalQueueConfig::default().with_size(50))
                    .define_job::<LargeBatchJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while LARGE_BATCH_CALL_COUNT.get().await < 100 {
            if start_time.elapsed().as_secs() > 30 {
                panic!(
                    "All jobs should have been executed by now, got {}",
                    LARGE_BATCH_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            LARGE_BATCH_CALL_COUNT.get().await,
            100,
            "All 100 jobs should have been executed"
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct RefetchDelayWithJobsJob {
    id: u32,
}

static REFETCH_WITH_JOBS_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for RefetchDelayWithJobsJob {
    const IDENTIFIER: &'static str = "refetch_delay_with_jobs_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        REFETCH_WITH_JOBS_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_processes_jobs_with_refetch_delay() {
    with_test_db(|test_db| async move {
        REFETCH_WITH_JOBS_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=5 {
            utils
                .add_job(RefetchDelayWithJobsJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(2)
                    .poll_interval(Duration::from_millis(200))
                    .local_queue(
                        LocalQueueConfig::default()
                            .with_size(10)
                            .with_refetch_delay(
                                RefetchDelayConfig::default()
                                    .with_duration(Duration::from_millis(100))
                                    .with_threshold(3)
                                    .with_max_abort_threshold(10),
                            ),
                    )
                    .define_job::<RefetchDelayWithJobsJob>()
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        let start_time = Instant::now();
        while REFETCH_WITH_JOBS_CALL_COUNT.get().await < 5 {
            if start_time.elapsed().as_secs() > 10 {
                panic!(
                    "Jobs should have been executed, got {}",
                    REFETCH_WITH_JOBS_CALL_COUNT.get().await
                );
            }
            sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            REFETCH_WITH_JOBS_CALL_COUNT.get().await,
            5,
            "All 5 jobs should have been executed with refetch delay configured"
        );

        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct PulseImmediateFetchJob {
    id: u32,
}

static PULSE_IMMEDIATE_CALL_COUNT: StaticCounter = StaticCounter::new();

impl TaskHandler for PulseImmediateFetchJob {
    const IDENTIFIER: &'static str = "pulse_immediate_fetch_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        PULSE_IMMEDIATE_CALL_COUNT.increment().await;
    }
}

#[tokio::test]
async fn local_queue_pulse_triggers_immediate_fetch() {
    with_test_db(|test_db| async move {
        PULSE_IMMEDIATE_CALL_COUNT.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(1)
                .poll_interval(Duration::from_secs(30))
                .local_queue(LocalQueueConfig::default().with_size(10))
                .listen_os_shutdown_signals(false)
                .define_job::<PulseImmediateFetchJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(500)).await;

        let start = Instant::now();
        utils
            .add_job(PulseImmediateFetchJob { id: 1 }, JobSpec::default())
            .await
            .expect("Failed to add job");

        while PULSE_IMMEDIATE_CALL_COUNT.get().await < 1 {
            if start.elapsed().as_secs() > 5 {
                panic!("Job should have been processed immediately via pulse, not after 30s poll");
            }
            sleep(Duration::from_millis(50)).await;
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(3),
            "Job should be processed quickly via pulse (took {:?}), not waiting for 30s poll_interval",
            elapsed
        );

        worker.request_shutdown();
        worker_fut.abort();
    })
    .await;
}

#[derive(Serialize, Deserialize)]
struct ReleaseWaitsJob {
    id: u32,
}

static RELEASE_WAITS_CALL_COUNT: StaticCounter = StaticCounter::new();
static RELEASE_WAITS_COMPLETED: StaticCounter = StaticCounter::new();

impl TaskHandler for ReleaseWaitsJob {
    const IDENTIFIER: &'static str = "release_waits_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        RELEASE_WAITS_CALL_COUNT.increment().await;
        sleep(Duration::from_millis(200)).await;
        RELEASE_WAITS_COMPLETED.increment().await;
    }
}

#[tokio::test]
async fn local_queue_release_waits_for_run_loop() {
    with_test_db(|test_db| async move {
        RELEASE_WAITS_CALL_COUNT.reset().await;
        RELEASE_WAITS_COMPLETED.reset().await;
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        for i in 1..=3 {
            utils
                .add_job(ReleaseWaitsJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(2)
                .local_queue(LocalQueueConfig::default().with_size(10))
                .listen_os_shutdown_signals(false)
                .define_job::<ReleaseWaitsJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        sleep(Duration::from_millis(500)).await;

        assert!(
            RELEASE_WAITS_CALL_COUNT.get().await > 0,
            "At least one job should have started"
        );

        worker.request_shutdown();

        let start = Instant::now();
        while !worker_fut.is_finished() {
            if start.elapsed().as_secs() > 10 {
                worker_fut.abort();
                panic!("Worker should have finished shutdown by now");
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            worker_fut.is_finished(),
            "Worker future should be finished after shutdown"
        );
    })
    .await;
}
