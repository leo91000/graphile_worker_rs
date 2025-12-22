use chrono::Utc;
use graphile_worker::{
    worker_utils::CleanupTask, IntoTaskHandlerResult, JobSpec, TaskHandler, WorkerContext,
};
use indoc::formatdoc;
use serde::{Deserialize, Serialize};

use crate::helpers::{with_test_db, StaticCounter};

mod helpers;

#[tokio::test]
async fn cleanup_with_delete_permafailed_jobs() {
    with_test_db(|test_db| async move {
        let worker_utils = test_db
            .create_worker_options()
            .init()
            .await
            .expect("Failed to create worker")
            .create_utils();

        // Create a selection of jobs
        let jobs = test_db.make_selection_of_jobs().await;
        let permafail_job_ids = [
            *jobs.failed_job.id(),
            *jobs.regular_job_1.id(),
            *jobs.regular_job_2.id(),
        ]
        .to_vec();

        // Permanently fail selected jobs
        let failed_jobs = worker_utils
            .permanently_fail_jobs(&permafail_job_ids, "TESTING!")
            .await
            .expect("Failed to permanently fail jobs");
        assert_eq!(
            failed_jobs.len(),
            permafail_job_ids.len(),
            "Should have permanently failed the specified jobs"
        );

        // Cleanup permanently failed jobs
        worker_utils
            .cleanup(&[CleanupTask::DeletePermenantlyFailedJobs])
            .await
            .expect("Failed to cleanup permanently failed jobs");

        // Fetch all jobs from the database
        let remaining_jobs = test_db.get_jobs().await;
        let remaining_job_ids = remaining_jobs.iter().map(|job| job.id).collect::<Vec<_>>();

        // Verify that only the non-permanently-failed jobs remain
        let expected_remaining_ids = [*jobs.locked_job.id(), *jobs.untouched_job.id()].to_vec();
        assert_eq!(
            remaining_job_ids, expected_remaining_ids,
            "Only non-permanently-failed jobs should remain"
        );
    })
    .await;
}

#[tokio::test]
async fn cleanup_with_gc_job_queues() {
    with_test_db(|test_db| async move {
        let worker_utils = test_db
            .create_worker_options()
            .init()
            .await
            .expect("Failed to create worker")
            .create_utils();

        let mut jobs = Vec::new();
        let specs = [
            ("worker1", "test", "test_job1"),
            ("worker2", "test2", "test_job2"),
            ("worker3", "test3", "test_job3"),
        ];

        let mut a = 0;
        let mut date = Utc::now();
        for (worker_id, queue_name, task_identifier) in specs.iter() {
            date -= chrono::Duration::minutes(1);
            a += 1;
            let job = worker_utils
                .add_raw_job(
                    task_identifier,
                    serde_json::json!({ "a": a }),
                    JobSpec {
                        queue_name: Some(queue_name.to_string()),
                        ..Default::default()
                    },
                )
                .await
                .expect("Failed to add job");
            jobs.push(job.clone());

            // Lock job using the large SQL query
            let sql = formatdoc!(
                r#"
                    WITH j AS (
                        UPDATE {schema}._private_jobs
                        SET locked_at = $1, locked_by = $2
                        WHERE id = $3
                        RETURNING *
                    ), q AS (
                        UPDATE {schema}._private_job_queues
                        SET locked_by = $2, locked_at = $1
                        FROM j
                        WHERE {schema}._private_job_queues.id = j.job_queue_id
                    )
                    SELECT * FROM j;
                "#,
                schema = "graphile_worker"
            );
            sqlx::query(&sql)
                .bind(date)
                .bind(worker_id)
                .bind(job.id())
                .execute(&test_db.test_pool)
                .await
                .expect("Failed to lock job");
        }

        // Unlock worker3 and complete the last job
        worker_utils
            .force_unlock_workers(&["worker3"])
            .await
            .expect("Failed to unlock worker3");

        let last_job = jobs.pop().expect("There should be at least one job");
        worker_utils
            .complete_jobs(&[*last_job.id()])
            .await
            .expect("Failed to complete the last job");

        // Perform GC_JOB_QUEUES cleanup
        worker_utils
            .cleanup(&[CleanupTask::GcJobQueues])
            .await
            .expect("Failed to cleanup job queues");

        // Fetch remaining job queues
        let remaining_queues = test_db.get_job_queues().await;
        let remaining_queue_names: Vec<_> = remaining_queues
            .iter()
            .map(|q| q.queue_name.clone())
            .collect();

        // Verify that only the expected job queues remain
        let expected_queues = vec!["test".to_string(), "test2".to_string()];
        assert_eq!(
            remaining_queue_names, expected_queues,
            "Only non-empty job queues should remain after cleanup"
        );
    })
    .await;
}

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

        // Complete the "test_job2"
        if let Some(job_id) = completed_job_id {
            worker_utils
                .complete_jobs(&[job_id])
                .await
                .expect("Failed to complete the job");
        }

        // Fetch task identifiers before cleanup
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

        // Perform GC_TASK_IDENTIFIERS cleanup
        worker_utils
            .cleanup(&[CleanupTask::GcTaskIdentifiers])
            .await
            .expect("Failed to cleanup task identifiers");

        // Fetch task identifiers after cleanup
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
