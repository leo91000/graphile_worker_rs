use crate::helpers::{with_test_db, SelectionOfJobs};

mod helpers;

#[tokio::test]
async fn permanently_fails_jobs_leaves_others_unaffected() {
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

        // Prepare job IDs for failing
        let job_ids_to_fail = vec![
            *failed_job.id(),
            *regular_job_1.id(),
            *locked_job.id(),
            *regular_job_2.id(),
        ];

        // Fail specified jobs
        let failed_jobs = worker_utils
            .permanently_fail_jobs(&job_ids_to_fail, "TESTING!")
            .await
            .expect("Failed to fail jobs");

        // Verify the failure of correct jobs and their attributes
        assert_eq!(
            failed_jobs.len(),
            3, // Excluding the locked job
            "Should have failed the specified jobs excluding the locked one"
        );

        for failed_job in &failed_jobs {
            assert_eq!(
                failed_job.last_error().as_deref(),
                Some("TESTING!"),
                "Failed job should have the correct last_error"
            );
            assert_eq!(
                failed_job.attempts(),
                failed_job.max_attempts(),
                "Failed job attempts should equal max_attempts"
            );
            assert!(
                failed_job.attempts() > &0,
                "Failed job attempts should be greater than 0"
            );
        }

        // Fetch remaining jobs
        let failed_job_ids: Vec<i64> = failed_jobs.iter().map(|job| *job.id()).collect();
        let remaining_jobs = test_db
            .get_jobs()
            .await
            .into_iter()
            .filter(|job| !failed_job_ids.contains(&job.id))
            .collect::<Vec<_>>();
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
