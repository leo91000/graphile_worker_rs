use super::*;

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
                .database(test_db.database.clone())
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
