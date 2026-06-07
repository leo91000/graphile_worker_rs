use super::super::*;

#[tokio::test]
async fn add_raw_jobs_handles_large_default_and_queued_batches() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct FastJob {
            value: usize,
        }

        impl TaskHandler for FastJob {
            const IDENTIFIER: &'static str = "large_fast_job";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        #[derive(Clone, Serialize, Deserialize)]
        struct OtherFastJob {
            value: usize,
        }

        impl TaskHandler for OtherFastJob {
            const IDENTIFIER: &'static str = "large_other_fast_job";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<FastJob>()
            .define_job::<OtherFastJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let fast_jobs: Vec<_> = (0..512)
            .map(|value| RawJobSpec {
                identifier: if value % 2 == 0 {
                    "large_fast_job".into()
                } else {
                    "large_other_fast_job".into()
                },
                payload: serde_json::json!({ "value": value }),
                spec: JobSpec::default(),
            })
            .collect();

        let utils = worker.create_utils();
        let added_fast_jobs = utils
            .add_raw_jobs(&fast_jobs)
            .await
            .expect("Failed to add large default batch");

        assert_eq!(added_fast_jobs.len(), 512);
        assert_eq!(
            added_fast_jobs
                .iter()
                .filter(|job| job.task_identifier() == "large_fast_job")
                .count(),
            256
        );
        assert_eq!(
            added_fast_jobs
                .iter()
                .filter(|job| job.task_identifier() == "large_other_fast_job")
                .count(),
            256
        );

        let queued_spec = JobSpecBuilder::new()
            .queue_name("large_borrowed_queue")
            .build();
        let queued_jobs: Vec<_> = (0..512)
            .map(|value| RawJobSpec {
                identifier: "large_fast_job".into(),
                payload: serde_json::json!({ "value": value }),
                spec: queued_spec.clone(),
            })
            .collect();

        let added_queued_jobs = utils
            .add_raw_jobs(&queued_jobs)
            .await
            .expect("Failed to add large queued batch");

        assert_eq!(added_queued_jobs.len(), 512);

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1024);
        assert_eq!(
            jobs.iter()
                .filter(|job| job.queue_name.as_deref() == Some("large_borrowed_queue"))
                .count(),
            512
        );
    })
    .await;
}
