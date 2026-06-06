use super::*;

#[tokio::test]
async fn runs_jobs_asynchronously() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static JOB_RX: OnceCell<Mutex<Option<oneshot::Receiver<()>>>> = OnceCell::const_new();

    #[derive(Deserialize, Serialize)]
    struct Job3 {
        a: i32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
            let mut rx = JOB_RX.get().unwrap().lock().await;
            if let Some(receiver) = rx.take() {
                receiver.await.unwrap();
            }
        }
    }

    helpers::with_test_db(|test_db| async move {
        let (tx, rx) = oneshot::channel::<()>();
        JOB_RX.set(Mutex::new(Some(rx))).unwrap();

        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule a job
        let start = Utc::now();
        worker
            .create_utils()
            .add_job(
                Job3 { a: 1 },
                JobSpec {
                    queue_name: Some("myqueue".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        let worker_id = worker.worker_id().to_owned();

        // Start the worker to pick up the job
        let worker_handle = spawn_local(async move {
            worker.run_once().await.expect("Failed to run worker");
        });

        // Wait for the job to be picked up but not completed
        let start_time = Instant::now();
        while JOB3_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job3 should have been picked up by now");
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            1,
            "Job should be in progress but not completed"
        );

        // Check job and queue state to ensure they reflect an in-progress job
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1, "There should be one job in progress");
        let job = &jobs[0];
        assert_eq!(job.task_identifier, "job3");
        assert_eq!(job.payload, serde_json::json!({ "a": 1 }));
        let now = Utc::now();
        let start_diff_ms = (job.run_at.timestamp_millis() - start.timestamp_millis()).abs();
        assert!(
            (job.run_at >= start || start_diff_ms <= 50) && job.run_at <= now,
            "Job run_at should be within expected range (>= start or within 50ms tolerance, and <= now). Diff from start: {}ms",
            start_diff_ms
        );
        assert_eq!(job.attempts, 1, "Job attempts should be incremented");
        let job_queues = test_db.get_job_queues().await;
        assert_eq!(
            job_queues.len(),
            1,
            "There should be one queue with a job in progress"
        );
        let q = &job_queues[0];
        assert_eq!(&q.queue_name, job.queue_name.as_ref().unwrap());
        assert_eq!(q.job_count, 1);
        assert!(q.locked_at.is_some(), "The job should be locked");
        let locked_at = q.locked_at.unwrap();
        let lock_start_diff_ms = (locked_at.timestamp_millis() - start.timestamp_millis()).abs();
        assert!(
            (locked_at >= start || lock_start_diff_ms <= 50) && locked_at <= Utc::now(),
            "The lock time should be within expected range (>= start or within 50ms tolerance, and <= now). Diff from start: {}ms",
            lock_start_diff_ms
        );
        assert_eq!(q.locked_by, Some(worker_id));

        // Complete the job
        tx.send(()).expect("Failed to send completion signal");
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

#[tokio::test]
async fn runs_jobs_in_parallel() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RXS: OnceCell<Mutex<Vec<oneshot::Receiver<()>>>> = OnceCell::const_new();
    RXS.set(Mutex::new(vec![])).unwrap();
    let mut txs = vec![];

    #[derive(Deserialize, Serialize)]
    struct Job3 {
        a: i32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            let rx = RXS
                .get()
                .expect("OnceCell should be set globally at the beginning of the test")
                .lock()
                .await
                .remove(0); // Obtain the receiver for this job
            JOB3_CALL_COUNT.increment().await;
            rx.await
                .expect("The receiver should not be dropped before the job completes");
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .concurrency(10)
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");
        let worker = Rc::new(worker);

        // Schedule 5 jobs in different queues
        for i in 1..=5 {
            let (tx, rx) = oneshot::channel::<()>();
            txs.push(tx);
            RXS.get().unwrap().lock().await.push(rx);

            worker
                .create_utils()
                .add_job(
                    Job3 { a: i },
                    JobSpec {
                        queue_name: Some(format!("queue_{}", i)),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");
        }

        let start = Utc::now();

        // Run 5 worker instances in parallel
        let mut handles = vec![];
        let worker_clone = worker.clone();
        handles.push(spawn_local(async move {
            worker_clone.run_once().await.expect("Failed to run worker");
        }));

        // Wait for all jobs are picked up
        let start_time = Instant::now();
        while JOB3_CALL_COUNT.get().await < 5 {
            if start_time.elapsed().as_secs() > 20 {
                panic!("Job3 should have been picked up by now");
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            5,
            "All jobs should be in progress"
        );

        // Verify jobs and queues state
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 5, "There should be 5 jobs in progress");

        let job_queues = test_db.get_job_queues().await;
        assert_eq!(job_queues.len(), 5, "There should be 5 job queues");
        for q in job_queues {
            assert_eq!(q.job_count, 1, "Each queue should have one job");

            let locked_at = q.locked_at.unwrap();
            let start_diff_ms = (locked_at.timestamp_millis() - start.timestamp_millis()).abs();
            assert!(
                locked_at >= start || start_diff_ms <= 50,
                "locked_at should be >= start or within 50ms tolerance, diff: {}ms",
                start_diff_ms
            );
            assert!(locked_at <= Utc::now(), "locked_at should be <= now");
        }

        // Complete all jobs
        for tx in txs {
            tx.send(()).expect("Failed to send completion signal");
        }

        // Wait for all worker instances to finish
        for handle in handles {
            handle.await.expect("Worker task failed");
        }

        // Verify all jobs completed
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            5,
            "All jobs should have completed"
        );

        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "All jobs should be removed after completion"
        );
    })
    .await;
}

#[tokio::test]
async fn single_worker_runs_jobs_in_series_purges_all_before_exit() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RXS: OnceCell<Mutex<VecDeque<oneshot::Receiver<()>>>> = OnceCell::const_new();
    RXS.set(Mutex::new(VecDeque::new())).unwrap();
    let mut txs = vec![];

    #[derive(Deserialize, Serialize)]
    struct Job3 {
        a: i32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            let rx = RXS.get().unwrap().lock().await.pop_front().unwrap(); // Obtain the receiver for the current job
            rx.await.unwrap(); // Wait for the signal to complete the job
            JOB3_CALL_COUNT.increment().await; // Increment counter after job completes
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule 5 jobs
        for _ in 1..=5 {
            let (tx, rx) = oneshot::channel::<()>();
            txs.push(tx);
            RXS.get().unwrap().lock().await.push_back(rx);

            worker
                .create_utils()
                .add_job(Job3 { a: 1 }, JobSpec::default())
                .await
                .expect("Failed to add job");
        }

        // Start the worker to pick up and run the jobs
        let worker_handle = spawn_local(async move {
            worker.run_once().await.expect("Failed to run worker");
        });

        // Sequentially complete each job and verify progress
        let mut i = 0;
        for tx in txs {
            i += 1;
            // Complete the current job
            tx.send(()).expect("Failed to send completion signal");

            let start_time = Instant::now();
            while JOB3_CALL_COUNT.get().await < i {
                assert!(
                    start_time.elapsed() < tokio::time::Duration::from_secs(5),
                    "Timed out waiting for job {} to complete",
                    i,
                );
                sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        // Wait for the worker to finish processing all jobs
        worker_handle.await.expect("Worker task failed");

        // Verify all jobs were completed
        assert_eq!(
            JOB3_CALL_COUNT.get().await,
            5,
            "All jobs should have completed"
        );

        // Check that all jobs are purged from the database
        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "All jobs should be removed after completion"
        );
    })
    .await;
}
