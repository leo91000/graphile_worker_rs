use super::*;

#[tokio::test]
async fn it_should_supports_future_scheduled_jobs() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: u32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        {
            let utils = worker.create_utils();

            utils
                .add_raw_job(
                    "job3",
                    json!({ "a": 1 }),
                    JobSpec {
                        run_at: Some(Utc::now() + chrono::Duration::seconds(3)),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");
        }

        // Run all the jobs now (none should run)
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 0);

        // Still not ready
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 0);

        // Make the job ready
        test_db.make_jobs_run_now("job3").await;
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        // It should be successful
        {
            let jobs = test_db.get_jobs().await;
            assert_eq!(jobs.len(), 0);
            let job_queues = test_db.get_job_queues().await;
            assert_eq!(job_queues.len(), 0);
        }
    })
    .await;
}

#[tokio::test]
async fn it_shoud_allow_update_of_pending_jobs() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: String,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        // Rust is more precise than postgres, so we need to remove the nanoseconds
        let run_at = Utc::now()
            .add(chrono::Duration::seconds(60))
            .with_nanosecond(0)
            .unwrap();
        let utils = worker.create_utils();
        // Schedule a future job - note incorrect payload
        utils
            .add_raw_job(
                "job3",
                json!({ "a": "wrong" }),
                JobSpec {
                    run_at: Some(run_at),
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        // Assert that it has an entry in jobs / job_queues
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];
        assert_eq!(job.run_at, run_at);

        // Run all jobs (none are ready)
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 0);

        // update the job to run immediately with correct payload
        let now = Utc::now().with_nanosecond(0).unwrap();
        utils
            .add_raw_job(
                "job3",
                json!({ "a": "right" }),
                JobSpec {
                    run_at: Some(now),
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        // Assert that it has updated the existing entry and not created a new one
        let updated_jobs = test_db.get_jobs().await;
        assert_eq!(updated_jobs.len(), 1);
        let updated_job = &updated_jobs[0];
        assert_eq!(job.id, updated_job.id);
        assert_eq!(updated_job.run_at, now);

        // Run the task
        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);
    })
    .await;
}

#[tokio::test]
async fn it_schedules_a_new_job_if_existing_is_completed() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: String,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();
        // Schedule a job to run immediately with job key "abc" and a payload.
        utils
            .add_raw_job(
                "job3",
                json!({ "a": "first" }),
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        // Run the worker once to process the job.
        worker.run_once().await.expect("Failed to run worker");

        // Assert the job has been called once.
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        // Attempt to update the job. Since the previous job is completed, it should schedule a new one.
        utils
            .add_raw_job(
                "job3",
                json!({ "a": "second" }),
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        // Run the worker again to process the new job.
        worker.run_once().await.expect("Failed to run worker");

        // Assert the job has been called twice, indicating the new job was scheduled and run.
        assert_eq!(JOB3_CALL_COUNT.get().await, 2);
    })
    .await;
}

#[tokio::test]
async fn schedules_a_new_job_if_existing_is_being_processed() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();
    static RX1: OnceCell<Mutex<Option<oneshot::Receiver<()>>>> = OnceCell::const_new();
    static RX2: OnceCell<Mutex<Option<oneshot::Receiver<()>>>> = OnceCell::const_new();

    #[derive(Deserialize, Serialize)]
    struct Job3 {
        a: String,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            let n = JOB3_CALL_COUNT.increment().await;
            match n {
                1 => {
                    let mut rx_opt = RX1.get().unwrap().lock().await;
                    if let Some(rx) = rx_opt.take() {
                        rx.await.unwrap();
                    }
                }
                2 => {
                    let mut rx_opt = RX2.get().unwrap().lock().await;
                    if let Some(rx) = rx_opt.take() {
                        rx.await.unwrap();
                    }
                }
                _ => unreachable!("Job3 should only be called twice"),
            };

            Ok::<_, ()>(())
        }
    }

    helpers::with_test_db(|test_db| async move {
        let (tx1, rx1) = oneshot::channel::<()>();
        let (tx2, rx2) = oneshot::channel::<()>();
        RX1.set(Mutex::new(Some(rx1))).unwrap();
        RX2.set(Mutex::new(Some(rx2))).unwrap();

        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        // Schedule the first job
        worker
            .create_utils()
            .add_raw_job(
                "job3",
                json!({ "a": "first" }),
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add first job");

        let worker = Rc::new(worker);

        tracing::info!("Starting worker");
        // Run the first job
        let run_once_1 = spawn_local({
            let worker = worker.clone();
            async move {
                worker.run_once().await.expect("Failed to run worker");
            }
        });

        tracing::info!("Waiting for first job to be picked up");
        // Wait for the first job to be picked up
        let start_time = Instant::now();
        while JOB3_CALL_COUNT.get().await < 1 {
            if start_time.elapsed().as_secs() > 5 {
                panic!("Job3 should have been executed by now");
            }
            sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(JOB3_CALL_COUNT.get().await, 1);

        // Schedule a new job with the same key while the first one is being processed
        worker
            .create_utils()
            .add_raw_job(
                "job3",
                json!({ "a": "second" }),
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add second job");

        // Complete the first job
        tx1.send(()).unwrap();

        run_once_1.await.expect("Failed to run worker");

        // Run the worker again to pick up the second job
        let run_once_2 = spawn_local({
            let worker = worker.clone();
            async move {
                worker.run_once().await.expect("Failed to run worker");
            }
        });

        // Complete the second job
        tx2.send(()).unwrap();

        run_once_2.await.expect("Failed to run worker");

        // Ensure both jobs have been processed
        assert_eq!(JOB3_CALL_COUNT.get().await, 2);
    })
    .await;
}

#[tokio::test]
async fn job_details_are_reset_if_not_specified_in_update() {
    static JOB3_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct Job3 {
        a: u32,
    }

    impl TaskHandler for Job3 {
        const IDENTIFIER: &'static str = "job3";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            JOB3_CALL_COUNT.increment().await;
        }
    }

    helpers::with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<Job3>()
            .init()
            .await
            .expect("Failed to create worker");

        let run_at = Utc::now()
            .add(Duration::seconds(3))
            .with_nanosecond(0)
            .unwrap();

        // Schedule a future job
        worker
            .create_utils()
            .add_job(
                Job3 { a: 1 },
                JobSpec {
                    queue_name: Some("queue1".into()),
                    run_at: Some(run_at),
                    max_attempts: Some(10),
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        // Assert initial job details
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        let original = &jobs[0];
        assert_eq!(original.attempts, 0);
        assert_eq!(original.key, Some("abc".to_string()));
        assert_eq!(original.max_attempts, 10);
        assert_eq!(original.payload, serde_json::json!({"a": 1}));
        assert_eq!(original.queue_name, Some("queue1".to_string()));
        assert_eq!(original.run_at, run_at);
        assert_eq!(original.task_identifier, "job3");

        // Update job without specifying new details
        worker
            .create_utils()
            .add_job(
                Job3 { a: 1 }, // maintaining payload for comparison
                JobSpec {
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to update job");

        // Check that omitted details have reverted to default values
        let updated_jobs = test_db.get_jobs().await;
        assert_eq!(updated_jobs.len(), 1);
        let updated_job = &updated_jobs[0];
        assert_eq!(updated_job.attempts, 0);
        assert_eq!(updated_job.key, Some("abc".to_string()));
        assert_eq!(updated_job.max_attempts, 25); // Default value for max_attempts
        assert_eq!(updated_job.payload, serde_json::json!({"a": 1})); // Payload remains unchanged
        assert_eq!(updated_job.queue_name, None); // Queue name should not change unless explicitly updated
        assert_ne!(updated_job.run_at, run_at); // `run_at` should revert to the default (current time)

        // Update job with new details
        let run_at2 = Utc::now()
            .add(Duration::seconds(5))
            .with_nanosecond(0)
            .unwrap();

        worker
            .create_utils()
            .add_job(
                Job3 { a: 2 },
                JobSpec {
                    queue_name: Some("queue2".into()),
                    run_at: Some(run_at2),
                    max_attempts: Some(100),
                    job_key: Some("abc".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to update job with new details");

        // Check that details have changed
        let final_jobs = test_db.get_jobs().await;
        assert_eq!(final_jobs.len(), 1);
        let final_job = &final_jobs[0];
        assert_eq!(final_job.attempts, 0);
        assert_eq!(final_job.key, Some("abc".to_string()));
        assert_eq!(final_job.max_attempts, 100);
        assert_eq!(final_job.payload, serde_json::json!({"a": 2}));
        assert_eq!(final_job.queue_name, Some("queue2".to_string()));
        assert_eq!(final_job.run_at, run_at2);
        assert_eq!(final_job.task_identifier, "job3"); // Assuming the task identifier remains unchanged
    })
    .await;
}
