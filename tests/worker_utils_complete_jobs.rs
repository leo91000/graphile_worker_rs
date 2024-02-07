use crate::helpers::{with_test_db, SelectionOfJobs};

mod helpers;

#[tokio::test]
async fn completes_jobs_leaves_others_unaffected() {
    with_test_db(|test_db| async move {
        let worker_utils = test_db
            .create_worker_options()
            .init()
            .await
            .expect("Failed to create worker")
            .create_utils();

        // Create a selection of jobs
        let SelectionOfJobs {
            failed_job,
            regular_job_1,
            locked_job,
            regular_job_2,
            untouched_job,
        } = test_db.make_selection_of_jobs().await;

        // Prepare job IDs for completion
        let job_ids_to_complete = vec![
            *failed_job.id(),
            *regular_job_1.id(),
            *locked_job.id(),
            *regular_job_2.id(),
        ];

        // Complete specified jobs
        let completed_jobs = worker_utils
            .complete_jobs(&job_ids_to_complete)
            .await
            .expect("Failed to complete jobs");

        // Extract IDs of completed jobs
        let mut completed_job_ids: Vec<i64> =
            completed_jobs.into_iter().map(|job| *job.id()).collect();
        completed_job_ids.sort();

        // Expected IDs to be completed (excluding the locked job)
        let mut expected_completed_ids =
            vec![*failed_job.id(), *regular_job_1.id(), *regular_job_2.id()];
        expected_completed_ids.sort();

        // Verify the completion of correct jobs
        assert_eq!(
            completed_job_ids, expected_completed_ids,
            "Completed job IDs should match expected"
        );

        // Fetch remaining jobs
        let remaining_jobs = test_db.get_jobs().await;
        let remaining_job_ids = remaining_jobs.iter().map(|job| job.id).collect::<Vec<_>>();

        // Verify remaining jobs are as expected
        assert_eq!(remaining_jobs.len(), 2, "There should be 2 remaining jobs");
        assert!(
            remaining_job_ids.contains(locked_job.id())
                && remaining_job_ids.contains(untouched_job.id()),
            "Remaining jobs should include the locked and untouched jobs"
        );
    })
    .await;
}
