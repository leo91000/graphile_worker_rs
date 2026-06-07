use super::*;

#[tokio::test]
async fn context_ext_exposes_utils_for_worker_management_methods() {
    with_test_db(|test_db| async move {
        #[derive(Clone, Serialize, Deserialize)]
        struct UtilityChildJob {
            value: i32,
        }

        impl TaskHandler for UtilityChildJob {
            const IDENTIFIER: &'static str = "ctx_ext_utility_child_job";

            async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
        }

        #[derive(Clone, Serialize, Deserialize)]
        struct UtilitySpawnerJob;

        impl TaskHandler for UtilitySpawnerJob {
            const IDENTIFIER: &'static str = "ctx_ext_utility_spawner_job";

            async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
                ctx.add_job(
                    UtilityChildJob { value: 1 },
                    JobSpecBuilder::new()
                        .run_at(chrono::Utc::now() + chrono::Duration::hours(1))
                        .build(),
                )
                .await
                .map_err(|error| error.to_string())?;

                let removable = ctx
                    .add_raw_job(
                        "ctx_ext_utility_child_job",
                        serde_json::json!({ "value": 2 }),
                        JobSpecBuilder::new().job_key("ctx-ext-remove").build(),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                let utils = ctx.utils();

                utils
                    .remove_job("ctx-ext-remove")
                    .await
                    .map_err(|error| error.to_string())?;

                let completable = ctx
                    .add_raw_job(
                        "ctx_ext_utility_child_job",
                        serde_json::json!({ "value": 3 }),
                        JobSpec::default(),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                utils
                    .complete_jobs(&[*completable.id()])
                    .await
                    .map_err(|error| error.to_string())?;

                let permanent = ctx
                    .add_raw_job(
                        "ctx_ext_utility_child_job",
                        serde_json::json!({ "value": 4 }),
                        JobSpecBuilder::new()
                            .run_at(chrono::Utc::now() + chrono::Duration::hours(1))
                            .max_attempts(1)
                            .build(),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                utils
                    .permanently_fail_jobs(&[*permanent.id()], "manual failure")
                    .await
                    .map_err(|error| error.to_string())?;

                let reschedulable = ctx
                    .add_raw_job(
                        "ctx_ext_utility_child_job",
                        serde_json::json!({ "value": 5 }),
                        JobSpecBuilder::new()
                            .run_at(chrono::Utc::now() + chrono::Duration::hours(1))
                            .build(),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                utils
                    .reschedule_jobs(
                        &[*reschedulable.id()],
                        RescheduleJobOptions {
                            attempts: Some(1),
                            priority: Some(5),
                            ..Default::default()
                        },
                    )
                    .await
                    .map_err(|error| error.to_string())?;

                utils
                    .force_unlock_workers(&["ctx-ext-worker"])
                    .await
                    .map_err(|error| error.to_string())?;
                utils
                    .cleanup(&[CleanupTask::GcJobQueues])
                    .await
                    .map_err(|error| error.to_string())?;
                utils.migrate().await.map_err(|error| error.to_string())?;

                assert_eq!(removable.key().as_deref(), Some("ctx-ext-remove"));

                Ok::<(), String>(())
            }
        }

        let worker = test_db
            .create_worker_options()
            .define_job::<UtilitySpawnerJob>()
            .define_job::<UtilityChildJob>()
            .init()
            .await
            .expect("Failed to create worker");

        worker
            .create_utils()
            .add_job(UtilitySpawnerJob, JobSpec::default())
            .await
            .expect("Failed to add utility spawner job");

        worker.run_once().await.expect("Failed to run worker");

        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 3);
        assert!(jobs
            .iter()
            .any(|job| job.payload.get("value").and_then(|value| value.as_i64()) == Some(1)));
        assert!(jobs
            .iter()
            .any(|job| job.last_error.as_deref() == Some("manual failure")));
        assert!(jobs.iter().any(
            |job| job.payload.get("value").and_then(|value| value.as_i64()) == Some(5)
                && job.priority == 5
                && job.attempts == 1
        ));
    })
    .await;
}
