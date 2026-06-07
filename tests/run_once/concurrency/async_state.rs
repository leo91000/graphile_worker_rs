use super::super::*;

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

        let worker_handle = spawn_local(async move {
            worker.run_once().await.expect("Failed to run worker");
        });

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

        tx.send(()).expect("Failed to send completion signal");
        worker_handle.await.expect("Worker task failed");

        assert_eq!(JOB3_CALL_COUNT.get().await, 1, "Job should have completed");

        let jobs_after = test_db.get_jobs().await;
        assert_eq!(
            jobs_after.len(),
            0,
            "Job should be removed after completion"
        );
    })
    .await;
}
