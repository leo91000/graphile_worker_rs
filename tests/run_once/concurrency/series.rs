use super::super::*;

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
            let rx = RXS.get().unwrap().lock().await.pop_front().unwrap();
            rx.await.unwrap();
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

        let worker_handle = spawn_local(async move {
            worker.run_once().await.expect("Failed to run worker");
        });

        let mut i = 0;
        for tx in txs {
            i += 1;
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

        worker_handle.await.expect("Worker task failed");

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
