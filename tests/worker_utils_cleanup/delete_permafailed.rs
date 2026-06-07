use graphile_worker::worker_utils::types::CleanupTask;

use crate::helpers::with_test_db;

#[tokio::test]
async fn cleanup_with_delete_permafailed_jobs() {
    with_test_db(|test_db| async move {
        let worker_utils = test_db
            .create_worker_options()
            .init()
            .await
            .expect("Failed to create worker")
            .create_utils();

        let jobs = test_db.make_selection_of_jobs().await;
        let permafail_job_ids = [
            *jobs.failed_job.id(),
            *jobs.regular_job_1.id(),
            *jobs.regular_job_2.id(),
        ]
        .to_vec();

        let failed_jobs = worker_utils
            .permanently_fail_jobs(&permafail_job_ids, "TESTING!")
            .await
            .expect("Failed to permanently fail jobs");
        assert_eq!(
            failed_jobs.len(),
            permafail_job_ids.len(),
            "Should have permanently failed the specified jobs"
        );

        worker_utils
            .cleanup(&[CleanupTask::DeletePermanentlyFailedJobs])
            .await
            .expect("Failed to cleanup permanently failed jobs");

        let remaining_jobs = test_db.get_jobs().await;
        let remaining_job_ids = remaining_jobs.iter().map(|job| job.id).collect::<Vec<_>>();

        let expected_remaining_ids = [*jobs.locked_job.id(), *jobs.untouched_job.id()].to_vec();
        assert_eq!(
            remaining_job_ids, expected_remaining_ids,
            "Only non-permanently-failed jobs should remain"
        );
    })
    .await;
}
