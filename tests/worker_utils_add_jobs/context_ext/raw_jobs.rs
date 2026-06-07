use super::*;

#[tokio::test]
async fn context_ext_add_raw_jobs_from_task_handler() {
    with_test_db(|test_db| async move {
        static SPAWNER_COUNTER: StaticCounter = StaticCounter::new();
        static CHILD_COUNTER: StaticCounter = StaticCounter::new();

        #[derive(Clone, Serialize, Deserialize)]
        struct ChildJob {
            value: i32,
        }

        impl TaskHandler for ChildJob {
            const IDENTIFIER: &'static str = "ctx_ext_raw_child_job";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                CHILD_COUNTER.increment().await;
            }
        }

        #[derive(Clone, Serialize, Deserialize)]
        struct RawSpawnerJob;

        impl TaskHandler for RawSpawnerJob {
            const IDENTIFIER: &'static str = "ctx_ext_raw_spawner_job";

            async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                SPAWNER_COUNTER.increment().await;

                let jobs = vec![
                    RawJobSpec {
                        identifier: "ctx_ext_raw_child_job".to_string(),
                        payload: serde_json::json!({ "value": 10 }),
                        spec: JobSpec::default(),
                    },
                    RawJobSpec {
                        identifier: "ctx_ext_raw_child_job".to_string(),
                        payload: serde_json::json!({ "value": 20 }),
                        spec: JobSpec::default(),
                    },
                ];

                ctx.add_raw_jobs(&jobs)
                    .await
                    .expect("Failed to add raw jobs from context");
            }
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<RawSpawnerJob>()
            .define_job::<ChildJob>()
            .init()
            .await
            .expect("Failed to create worker");

        worker
            .create_utils()
            .add_job(RawSpawnerJob, JobSpec::default())
            .await
            .expect("Failed to add spawner job");

        worker.run_once().await.expect("Failed to run worker");

        assert_eq!(SPAWNER_COUNTER.get().await, 1, "Spawner should have run");

        worker
            .run_once()
            .await
            .expect("Failed to run worker second time");

        assert_eq!(
            CHILD_COUNTER.get().await,
            2,
            "All child jobs should have run"
        );
    })
    .await;
}
