use graphile_worker::{IntoTaskHandlerResult, JobSpec, TaskHandler, Worker, WorkerContext};
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

#[tokio::test]
async fn test_complete_job_batch_delay() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counter = CompletedCounter::new();

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(4)
                .poll_interval(Duration::from_millis(50))
                .complete_job_batch_delay(Duration::from_millis(10))
                .add_extension(counter.clone())
                .define_job::<SuccessJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_clone = worker.clone();
        let worker_handle = spawn_local(async move {
            worker_clone.run().await.expect("Failed to run worker");
        });

        for i in 0..10 {
            utils
                .add_job(SuccessJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();
        while counter.get() < 10 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!(
                    "Jobs should have completed by now, only {} completed",
                    counter.get()
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(counter.get(), 10, "All 10 jobs should have completed");

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}

#[tokio::test]
async fn test_fail_job_batch_delay() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counter = CompletedCounter::new();

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(4)
                .poll_interval(Duration::from_millis(50))
                .fail_job_batch_delay(Duration::from_millis(10))
                .add_extension(counter.clone())
                .define_job::<FailJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_clone = worker.clone();
        let worker_handle = spawn_local(async move {
            worker_clone.run().await.expect("Failed to run worker");
        });

        for i in 0..5 {
            utils
                .add_job(
                    FailJob { id: i },
                    JobSpec::builder().max_attempts(1).build(), // No retries
                )
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();
        while counter.get() < 5 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!(
                    "Jobs should have been attempted by now, only {} attempted",
                    counter.get()
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(counter.get(), 5, "All 5 jobs should have been attempted");

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}

#[tokio::test]
async fn test_both_batchers_together() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let success_counter = CompletedCounter::new();
        let fail_counter = CompletedCounter::new();

        #[derive(Clone, Debug)]
        struct SuccessCounter(CompletedCounter);

        #[derive(Clone, Debug)]
        struct FailCounter(CompletedCounter);

        #[derive(Serialize, Deserialize)]
        struct MixedSuccessJob {
            id: u32,
        }

        impl TaskHandler for MixedSuccessJob {
            const IDENTIFIER: &'static str = "mixed_success_job";

            async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                if let Some(counter) = ctx.get_ext::<SuccessCounter>() {
                    counter.0.increment();
                }
                Ok::<(), String>(())
            }
        }

        #[derive(Serialize, Deserialize)]
        struct MixedFailJob {
            id: u32,
        }

        impl TaskHandler for MixedFailJob {
            const IDENTIFIER: &'static str = "mixed_fail_job";

            async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                if let Some(counter) = ctx.get_ext::<FailCounter>() {
                    counter.0.increment();
                }
                Err::<(), String>(format!("Job {} failed", self.id))
            }
        }

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(8)
                .poll_interval(Duration::from_millis(50))
                .complete_job_batch_delay(Duration::from_millis(10))
                .fail_job_batch_delay(Duration::from_millis(10))
                .add_extension(SuccessCounter(success_counter.clone()))
                .add_extension(FailCounter(fail_counter.clone()))
                .define_job::<MixedSuccessJob>()
                .define_job::<MixedFailJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_clone = worker.clone();
        let worker_handle = spawn_local(async move {
            worker_clone.run().await.expect("Failed to run worker");
        });

        for i in 0..5 {
            utils
                .add_job(MixedSuccessJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
            utils
                .add_job(
                    MixedFailJob { id: i },
                    JobSpec::builder().max_attempts(1).build(),
                )
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();
        while success_counter.get() < 5 || fail_counter.get() < 5 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!(
                    "Jobs should have completed by now, success: {}, fail: {}",
                    success_counter.get(),
                    fail_counter.get()
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(success_counter.get(), 5);
        assert_eq!(fail_counter.get(), 5);

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}

#[tokio::test]
async fn test_shutdown_flushes_pending_completions() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counter = CompletedCounter::new();

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(4)
                .poll_interval(Duration::from_millis(50))
                .complete_job_batch_delay(Duration::from_millis(100))
                .add_extension(counter.clone())
                .define_job::<SuccessJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_clone = worker.clone();
        let worker_handle = spawn_local(async move {
            worker_clone.run().await.expect("Failed to run worker");
        });

        for i in 0..3 {
            utils
                .add_job(SuccessJob { id: i }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();
        while counter.get() < 3 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!("Jobs should have run by now");
            }
            sleep(Duration::from_millis(50)).await;
        }

        worker.request_shutdown();
        let _ = worker_handle.await;

        let remaining_jobs: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM graphile_worker._private_jobs")
                .fetch_one(&test_db.test_pool)
                .await
                .expect("Failed to count jobs");

        assert_eq!(
            remaining_jobs.0, 0,
            "All jobs should have been completed and removed from the database"
        );
    })
    .await;
}

#[tokio::test]
async fn test_retryable_failures_processed_individually() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let counter = CompletedCounter::new();

        let worker = Arc::new(
            Worker::options()
                .pg_pool(test_db.test_pool.clone())
                .concurrency(4)
                .poll_interval(Duration::from_millis(50))
                .fail_job_batch_delay(Duration::from_millis(10))
                .add_extension(counter.clone())
                .define_job::<FailJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_clone = worker.clone();
        let worker_handle = spawn_local(async move {
            worker_clone.run().await.expect("Failed to run worker");
        });

        utils
            .add_job(
                FailJob { id: 1 },
                JobSpec::builder().max_attempts(3).build(), // Will retry
            )
            .await
            .expect("Failed to add job");

        let start = Instant::now();
        while counter.get() < 1 {
            if start.elapsed() > Duration::from_secs(5) {
                panic!("Job should have been attempted by now");
            }
            sleep(Duration::from_millis(50)).await;
        }

        sleep(Duration::from_millis(200)).await;

        let job: Option<(i64, i16, i16)> = sqlx::query_as(
            "SELECT id, attempts, max_attempts FROM graphile_worker._private_jobs LIMIT 1",
        )
        .fetch_optional(&test_db.test_pool)
        .await
        .expect("Failed to query job");

        assert!(job.is_some(), "Job should still exist for retry");
        let (_, attempts, max_attempts) = job.unwrap();
        assert_eq!(attempts, 1, "Job should have 1 attempt");
        assert_eq!(max_attempts, 3, "Job should have max_attempts=3");

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}
