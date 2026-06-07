use super::*;

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
