use graphile_worker::{
    worker_utils::types::CleanupTask, IntoTaskHandlerResult, JobSpec, TaskHandler, WorkerContext,
};
use serde::{Deserialize, Serialize};

use crate::helpers::{with_test_db, StaticCounter};

#[tokio::test]
async fn cleanup_with_gc_task_identifiers() {
    with_test_db(|test_db| async move {
        let worker_utils = test_db
            .create_worker_options()
            .init()
            .await
            .expect("Failed to create worker")
            .create_utils();

        let task_identifiers = ["job3", "test_job1", "test_job2", "test_job3"];
        let mut completed_job_id = None;

        for task_identifier in task_identifiers.iter() {
            let job = worker_utils
                .add_raw_job(task_identifier, serde_json::json!({}), JobSpec::default())
                .await
                .expect("Failed to add job");

            if *task_identifier == "test_job2" {
                completed_job_id = Some(*job.id());
            }
        }

        if let Some(job_id) = completed_job_id {
            worker_utils
                .complete_jobs(&[job_id])
                .await
                .expect("Failed to complete the job");
        }

        let mut tasks_before = test_db.get_tasks().await;
        tasks_before.sort();

        assert_eq!(
            tasks_before,
            {
                let mut vec = vec!["job3", "test_job1", "test_job2", "test_job3"];
                vec.sort();
                vec
            },
            "All task identifiers should be present before cleanup"
        );

        worker_utils
            .cleanup(&[CleanupTask::GcTaskIdentifiers])
            .await
            .expect("Failed to cleanup task identifiers");

        let mut tasks_after = test_db.get_tasks().await;
        tasks_after.sort();

        assert_eq!(
            tasks_after,
            {
                let mut vec = vec!["job3", "test_job1", "test_job3"];
                vec.sort();
                vec
            },
            "Only relevant task identifiers should remain after cleanup"
        );
    })
    .await;
}

#[tokio::test]
async fn gc_task_identifiers_preserves_known_identifiers_for_horizontal_scaling() {
    static CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize)]
    struct RefreshTestJob;

    impl TaskHandler for RefreshTestJob {
        const IDENTIFIER: &'static str = "refresh_test_job";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            CALL_COUNT.increment().await;
        }
    }

    with_test_db(|test_db| async move {
        let worker = test_db
            .create_worker_options()
            .define_job::<RefreshTestJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();

        utils
            .add_job(RefreshTestJob, JobSpec::default())
            .await
            .expect("Failed to add first job");

        worker.run_once().await.expect("Failed to run worker");
        assert_eq!(CALL_COUNT.get().await, 1, "First job should have run");

        let jobs_after_first_run = test_db.get_jobs().await;
        assert!(jobs_after_first_run.is_empty(), "Job should be completed");

        let task_id_before = test_db.get_task_id("refresh_test_job").await;

        utils
            .cleanup(&[CleanupTask::GcTaskIdentifiers])
            .await
            .expect("Failed to cleanup");

        let task_id_after = test_db.get_task_id("refresh_test_job").await;
        assert_eq!(
            task_id_before, task_id_after,
            "Task ID should stay the same (preserved for horizontal scaling safety)"
        );

        utils
            .add_job(RefreshTestJob, JobSpec::default())
            .await
            .expect("Failed to add second job");

        worker
            .run_once()
            .await
            .expect("Failed to run worker second time");
        assert_eq!(CALL_COUNT.get().await, 2, "Second job should have run");

        let jobs_after_second_run = test_db.get_jobs().await;
        assert!(
            jobs_after_second_run.is_empty(),
            "Second job should be completed"
        );
    })
    .await;
}

/// Explicit keeps protect other workers without registering their handlers locally.
#[tokio::test]
async fn explicit_keep_preserves_external_identity_without_changing_local_cache() {
    with_test_db(|db| async move {
        #[derive(Serialize, Deserialize)]
        struct LocalJob;
        impl TaskHandler for LocalJob {
            const IDENTIFIER: &'static str = "local_registered";
            async fn run(self, _: WorkerContext) -> impl IntoTaskHandlerResult {}
        }
        let worker = db.create_worker_options().define_job::<LocalJob>().init().await.unwrap();
        let utils = worker.create_utils();
        sqlx::query("INSERT INTO graphile_worker._private_tasks (identifier) VALUES ('external_keep'), ('external_remove')")
            .execute(&db.test_pool).await.unwrap();
        let before = db.get_task_id("external_keep").await;
        utils.cleanup_with_keep(&[CleanupTask::GcTaskIdentifiers], &["external_keep".into(), "external_keep".into(), "not_registered".into()])
            .await.unwrap();
        let mut tasks = db.get_tasks().await;
        tasks.sort();
        assert_eq!(tasks, ["external_keep", "local_registered"]);
        assert_eq!(db.get_task_id("external_keep").await, before);
        assert_eq!(worker.task_details().task_names().await, ["local_registered"]);
        // Keeps apply to this cleanup invocation only and do not enter the cache.
        utils.cleanup(&[CleanupTask::GcTaskIdentifiers]).await.unwrap();
        assert_eq!(db.get_tasks().await, ["local_registered"]);
    }).await;
}
