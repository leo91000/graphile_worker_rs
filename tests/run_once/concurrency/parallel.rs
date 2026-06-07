use super::super::*;

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
                .remove(0);
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

        let mut handles = vec![];
        let worker_clone = worker.clone();
        handles.push(spawn_local(async move {
            worker_clone.run_once().await.expect("Failed to run worker");
        }));

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

        for tx in txs {
            tx.send(()).expect("Failed to send completion signal");
        }

        for handle in handles {
            handle.await.expect("Worker task failed");
        }

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
