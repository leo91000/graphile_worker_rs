use super::*;

#[tokio::test]
async fn pending_jobs_can_be_removed() {
    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: u32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job
        worker
            .create_utils()
            .add_job(
                Job3 { a: 1 },
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        // Assert that it has an entry in jobs
        let jobs_before_removal = test_db.get_jobs().await;
        assert_eq!(
            jobs_before_removal.len(),
            1,
            "There should be one job scheduled"
        );

        // Remove the job
        worker
            .create_utils()
            .remove_job("abc")
            .await
            .expect("Failed to remove job");

        // Check there are no jobs
        let jobs_after_removal = test_db.get_jobs().await;
        assert_eq!(
            jobs_after_removal.len(),
            0,
            "There should be no jobs scheduled after removal"
        );
    })
    .await;
}

#[tokio::test]
async fn jobs_in_progress_cannot_be_removed() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RX: OnceCell<Mutex<Option<oneshot::Receiver<()>>>> = OnceCell::const_new();

    #[derive(Deserialize, Serialize)]
    struct Job3 {
        a: i32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
            // Wait on the receiver, simulating a deferred operation
            let mut rx_mutex_guard = RX.get().unwrap().lock().await;
            if let Some(rx) = rx_mutex_guard.take() {
                rx.await.unwrap();
            }
        }
    }

    helpers::with_test_db(|test_db| async move {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        RX.set(Mutex::new(Some(rx))).unwrap();

        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job
        let utils = worker.create_utils();
        utils
            .add_job(
                Job3 { a: 123 },
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        // Check it was inserted
        let jobs_before = test_db.get_jobs().await;
        assert_eq!(jobs_before.len(), 1, "Job should be scheduled");

        // Run the worker in a separate task to pick up the job
        let worker_handle = spawn_local(async move {
            worker.run_once().await.expect("Failed to run worker");
        });

        // Wait for the job to be picked up
        let start_time = Instant::now();
        while JOB3_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job3 should have been picked up by now");
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(JOB3_CALL_COUNT.get().await, 1, "Job should be in progress");

        // Attempt to remove the job
        utils
            .remove_job("abc")
            .await
            .expect("Failed to attempt job removal");

        // Check it was not removed
        let jobs_during = test_db.get_jobs().await;
        assert_eq!(
            jobs_during.len(),
            1,
            "Job should not be removed while in progress"
        );

        // Complete the job
        tx.send(()).expect("Failed to send completion signal");

        // Wait for the worker task to complete
        worker_handle.await.expect("Worker task failed");

        // Verify the job completed
        assert_eq!(JOB3_CALL_COUNT.get().await, 1, "Job should have completed");

        // Check the job is removed after completion
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "Job should be removed after completion"
        );
    })
    .await;
}
