use super::*;

#[tokio::test]
async fn background_sweeper_recovers_job_from_dead_worker() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 5 }, JobSpec::default())
            .await
            .expect("failed to add job");

        let worker_a = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_secs(60))
                .sweep_threshold(Duration::from_millis(250))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .define_job::<LongJob>()
                .init()
                .await
                .expect("failed to init worker A"),
        );

        let worker_a_id = worker_a.worker_id().to_string();
        let worker_a_for_run = Arc::clone(&worker_a);
        let worker_a_fut = tokio::task::spawn(async move {
            let _ = worker_a_for_run.run().await;
        });

        let start = Instant::now();
        loop {
            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.id == *job.id() && j.locked_by.as_deref() == Some(&worker_a_id))
            {
                break;
            }

            assert!(
                start.elapsed() <= Duration::from_secs(5),
                "worker A should lock the job before the test timeout"
            );
            sleep(Duration::from_millis(50)).await;
        }

        worker_a_fut.abort();
        let _ = worker_a_fut.await;

        let sweeper = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_millis(100))
                .sweep_threshold(Duration::from_millis(250))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .init()
                .await
                .expect("failed to init sweeper worker"),
        );

        let sweeper_for_run = Arc::clone(&sweeper);
        let sweeper_fut = tokio::task::spawn(async move { sweeper_for_run.run().await });

        let start = Instant::now();
        loop {
            let jobs = test_db.get_jobs().await;
            let recovered = jobs
                .iter()
                .find(|j| j.id == *job.id())
                .is_some_and(|j| j.locked_by.is_none() && j.attempts == 0);
            if recovered {
                break;
            }

            if start.elapsed() > Duration::from_secs(8) {
                sweeper.request_shutdown();
                let _ = sweeper_fut.await;
                panic!("background sweeper should recover the dead worker job");
            }

            sleep(Duration::from_millis(100)).await;
        }

        sweeper.request_shutdown();
        sweeper_fut
            .await
            .expect("sweeper task should not panic")
            .expect("sweeper should shut down cleanly");
    })
    .await;
}
