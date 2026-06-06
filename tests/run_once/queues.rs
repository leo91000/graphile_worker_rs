use super::*;

#[tokio::test]
async fn jobs_added_to_the_same_queue_will_be_ran_serially_even_if_multiple_workers() {
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
        let worker = Rc::new(worker);

        // Schedule 5 jobs to the same queue
        for _ in 1..=5 {
            let (tx, rx) = oneshot::channel::<()>();
            txs.push(tx);
            RXS.get().unwrap().lock().await.push_back(rx);

            worker
                .create_utils()
                .add_job(
                    Job3 { a: 1 },
                    JobSpecBuilder::new().queue_name("serial").build(),
                )
                .await
                .expect("Failed to add job");
        }

        // Start multiple worker instances to process the jobs
        let mut handles = vec![];
        for _ in 0..3 {
            let worker = worker.clone();
            handles.push(spawn_local(async move {
                worker.run_once().await.expect("Failed to run worker");
            }));
        }

        // Sequentially complete each job and verify progress
        for i in 1..=5 {
            // Give other workers a chance to interfere
            sleep(tokio::time::Duration::from_millis(50)).await;

            // Complete the current job
            txs.remove(0)
                .send(())
                .expect("Failed to send completion signal");

            // Wait for the job to be picked up
            let start_time = Instant::now();
            while JOB3_CALL_COUNT.get().await < i {
                if start_time.elapsed().as_secs() > 5 {
                    panic!("Job3 should have been picked up by now");
                }
                sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        // Wait for all worker instances to finish
        for handle in handles {
            handle.await.expect("Worker task failed");
        }

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
