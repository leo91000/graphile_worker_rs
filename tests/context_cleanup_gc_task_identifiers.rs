use graphile_worker::{
    worker_utils::CleanupTask, IntoTaskHandlerResult, JobSpec, TaskHandler, WorkerContext,
    WorkerContextExt,
};
use serde::{Deserialize, Serialize};

use crate::helpers::{with_test_db, StaticCounter};

mod helpers;

#[tokio::test]
async fn ctx_cleanup_refreshes_task_identifiers_and_jobs_run() {
    static EVENT_HANDLER_CALL_COUNT: StaticCounter = StaticCounter::new();

    #[derive(Serialize, Deserialize, Clone)]
    struct CleanupJob;

    impl TaskHandler for CleanupJob {
        const IDENTIFIER: &'static str = "cleanup";

        async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            ctx.cleanup(&[CleanupTask::GcTaskIdentifiers])
                .await
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }
    }

    #[derive(Serialize, Deserialize, Clone)]
    struct EventHandlerJob;

    impl TaskHandler for EventHandlerJob {
        const IDENTIFIER: &'static str = "event_handler";

        async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
            EVENT_HANDLER_CALL_COUNT.increment().await;
            Ok::<(), String>(())
        }
    }

    with_test_db(|test_db| async move {
        EVENT_HANDLER_CALL_COUNT.reset().await;

        let worker = test_db
            .create_worker_options()
            .define_job::<CleanupJob>()
            .define_job::<EventHandlerJob>()
            .init()
            .await
            .expect("Failed to create worker");

        let utils = worker.create_utils();

        utils
            .add_job(CleanupJob, JobSpec::default())
            .await
            .expect("Failed to add cleanup job");

        worker.run_once().await.expect("Failed to run cleanup job");

        let mut tasks_after_cleanup = test_db.get_tasks().await;
        tasks_after_cleanup.sort();

        assert_eq!(
            tasks_after_cleanup,
            vec!["cleanup".to_string(), "event_handler".to_string()],
            "Task identifiers should be preserved after cleanup"
        );

        utils
            .add_job(EventHandlerJob, JobSpec::default())
            .await
            .expect("Failed to add event handler job");

        worker
            .run_once()
            .await
            .expect("Failed to run event handler job");

        assert_eq!(
            EVENT_HANDLER_CALL_COUNT.get().await,
            1,
            "Event handler job should run without restarting the worker"
        );

        let remaining_jobs = test_db.get_jobs().await;
        assert!(remaining_jobs.is_empty(), "Job should be completed");
    })
    .await;
}
