use std::sync::Arc;

use graphile_worker::{IntoTaskHandlerResult, JobSpec, TaskHandler, Worker, WorkerContext};
use serde::{Deserialize, Serialize};
use tokio::{
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
                .pg_pool(test_db.test_pool.clone())
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
