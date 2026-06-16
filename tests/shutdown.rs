use std::sync::Arc;

use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, TaskHandler, Worker, WorkerContext, WorkerShutdownConfig,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::Notify,
    task::spawn_local,
    time::{Duration, Instant},
};

use crate::helpers::{with_test_db, StaticCounter};

mod helpers;

#[tokio::test]
async fn request_shutdown_executes_scheduled_jobs() {
    static JOB_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct ShutdownJob;

    impl TaskHandler for ShutdownJob {
        const IDENTIFIER: &'static str = "shutdown_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB_CALL_COUNT.increment().await;
        }
    }

    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(3)
                .listen_os_shutdown_signals(false)
                .define_job::<ShutdownJob>()
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_handle = spawn_local({
            let worker = worker.clone();
            async move {
                worker.run().await.expect("Worker run failed");
            }
        });

        let job_count = 5;
        for _ in 0..job_count {
            utils
                .add_job(ShutdownJob, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        let start = Instant::now();
        while JOB_CALL_COUNT.get().await < job_count {
            if start.elapsed().as_secs() > 5 {
                panic!("Jobs should have been processed before shutdown");
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        worker.request_shutdown();

        tokio::time::timeout(Duration::from_secs(2), worker_handle)
            .await
            .expect("Worker did not shut down after request")
            .expect("Worker task panicked");

        let remaining_jobs = test_db.get_jobs().await;
        assert!(
            remaining_jobs.is_empty(),
            "Expected no remaining jobs, found {}",
            remaining_jobs.len()
        );

        assert_eq!(
            JOB_CALL_COUNT.get().await,
            job_count,
            "All scheduled jobs should have run before shutdown"
        );
    })
    .await;
}

#[tokio::test]
async fn custom_shutdown_signal_stops_worker_run() {
    with_test_db(|test_db| async move {
        let shutdown_notify = Arc::new(Notify::new());
        let shutdown = WorkerShutdownConfig::default()
            .listen_os_shutdown_signals(false)
            .shutdown_signal({
                let shutdown_notify = shutdown_notify.clone();
                async move {
                    shutdown_notify.notified().await;
                }
            });

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .worker_shutdown(shutdown)
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_handle = spawn_local({
            let worker = worker.clone();
            async move {
                worker.run().await.expect("Worker run failed");
            }
        });

        shutdown_notify.notify_one();

        tokio::time::timeout(Duration::from_secs(2), worker_handle)
            .await
            .expect("Worker did not shut down after custom shutdown signal")
            .expect("Worker task panicked");
    })
    .await;
}

#[tokio::test]
async fn request_shutdown_still_works_with_pending_custom_signal() {
    with_test_db(|test_db| async move {
        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .listen_os_shutdown_signals(false)
                .shutdown_signal(std::future::pending::<()>())
                .init()
                .await
                .expect("Failed to create worker"),
        );

        let worker_handle = spawn_local({
            let worker = worker.clone();
            async move {
                worker.run().await.expect("Worker run failed");
            }
        });

        worker.request_shutdown();

        tokio::time::timeout(Duration::from_secs(2), worker_handle)
            .await
            .expect("Worker did not shut down after request_shutdown")
            .expect("Worker task panicked");
    })
    .await;
}
