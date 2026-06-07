use super::*;

#[tokio::test]
async fn test_batchers_emit_lifecycle_hooks() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        let attempts = CompletedCounter::new();
        let plugin = BatcherHooksPlugin::new();
        let hook_counters = plugin.counters();

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(4)
                .poll_interval(Duration::from_millis(50))
                .complete_job_batch_delay(Duration::from_millis(10))
                .fail_job_batch_delay(Duration::from_millis(10))
                .add_extension(attempts.clone())
                .add_plugin(plugin)
                .define_job::<SuccessJob>()
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
            .add_job(SuccessJob { id: 1 }, JobSpec::default())
            .await
            .expect("Failed to add success job");
        utils
            .add_job(
                SuccessJob { id: 2 },
                JobSpec::builder().queue_name("hooked_complete_queue").build(),
            )
            .await
            .expect("Failed to add queued success job");
        utils
            .add_job(FailJob { id: 1 }, JobSpec::builder().max_attempts(3).build())
            .await
            .expect("Failed to add retryable fail job");
        utils
            .add_job(
                FailJob { id: 2 },
                JobSpec::builder()
                    .queue_name("hooked_fail_queue")
                    .max_attempts(1)
                    .build(),
            )
            .await
            .expect("Failed to add permanent fail job");

        let start = Instant::now();
        while hook_counters.complete.load(Ordering::SeqCst) < 2
            || hook_counters.fail.load(Ordering::SeqCst) < 1
            || hook_counters.permanent.load(Ordering::SeqCst) < 1
        {
            if start.elapsed() > Duration::from_secs(5) {
                panic!(
                    "Batch hooks should have fired by now, complete: {}, fail: {}, permanent: {}, attempts: {}",
                    hook_counters.complete.load(Ordering::SeqCst),
                    hook_counters.fail.load(Ordering::SeqCst),
                    hook_counters.permanent.load(Ordering::SeqCst),
                    attempts.get()
                );
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(hook_counters.complete.load(Ordering::SeqCst), 2);
        assert_eq!(hook_counters.fail.load(Ordering::SeqCst), 1);
        assert_eq!(hook_counters.permanent.load(Ordering::SeqCst), 1);

        worker.request_shutdown();
        let _ = worker_handle.await;
    })
    .await;
}
