use graphile_worker::Worker;
use tokio::{
    task::spawn_local,
    time::{sleep, Duration, Instant},
};

use crate::helpers::{with_test_db, StaticCounter};

mod helpers;

#[tokio::test]
async fn it_will_execute_jobs_as_they_come_up_and_exits_cleanly() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Job3Args {
        a: u32,
    }

    #[graphile_worker::task]
    async fn job3(_args: Job3Args, _: graphile_worker::WorkerContext) -> Result<(), ()> {
        JOB3_CALL_COUNT.increment().await;
        Ok(())
    }

    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        // Create a worker that will execute the job
        let worker_fut = spawn_local({
            let test_pool = test_db.test_pool.clone();
            async move {
                Worker::options()
                    .pg_pool(test_pool)
                    .concurrency(3)
                    .define_job(job3)
                    .init()
                    .await
                    .expect("Failed to create worker")
                    .run()
                    .await
                    .expect("Failed to run worker");
            }
        });

        // Schedule 5 jobs and wait for them to be processed
        for i in 1..=5 {
            utils
                .add_job::<job3>(Job3Args { a: i }, None)
                .await
                .expect("Failed to add job");

            // Sleep until the job counter increment to 1
            let start_time = Instant::now();
            while JOB3_CALL_COUNT.get().await < i {
                if start_time.elapsed().as_secs() > 5 {
                    panic!("Job3 should have been executed by now");
                }
                sleep(Duration::from_millis(100)).await;
            }

            assert_eq!(
                JOB3_CALL_COUNT.get().await,
                i,
                "Job3 should have been executed {} times",
                i
            );
        }

        sleep(Duration::from_secs(1)).await;
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            5,
            "Job3 should have been executed 5 times"
        );

        // Abort the worker
        worker_fut.abort();
    })
    .await;
}
