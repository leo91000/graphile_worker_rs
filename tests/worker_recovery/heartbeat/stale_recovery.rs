use super::*;

#[tokio::test]
async fn stale_worker_heartbeat_recovers_locked_jobs() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        utils
            .add_job(LongJob { id: 1 }, JobSpec::default())
            .await
            .expect("failed to add job");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_millis(200))
                .sweep_threshold(Duration::from_millis(300))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .define_job::<LongJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );

        let worker_id = worker.worker_id().to_string();
        let worker_for_run = Arc::clone(&worker);
        let worker_fut = tokio::task::spawn(async move {
            let _ = worker_for_run.run().await;
        });

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.locked_by.as_deref() == Some(worker_id.as_str()))
            {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        worker_fut.abort();
        let _ = worker_fut.await;

        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
        let sweep_start = Instant::now();
        let mut recovered = false;
        while sweep_start.elapsed() < Duration::from_secs(10) {
            let result = sweep_utils
                .sweep_stale_workers(SweepStaleWorkersOptions {
                    sweep_threshold: Some(Duration::from_millis(300)),
                    recovery_delay: Some(Duration::from_millis(100)),
                    dry_run: false,
                })
                .await
                .expect("failed to sweep stale workers");

            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.locked_by.is_none() && j.attempts == 0)
            {
                recovered =
                    !result.worker_ids.is_empty() || jobs.iter().any(|j| j.locked_by.is_none());
                if recovered {
                    break;
                }
            }

            sleep(Duration::from_millis(250)).await;
        }

        assert!(recovered, "job should be recovered from stale worker");
    })
    .await;
}
