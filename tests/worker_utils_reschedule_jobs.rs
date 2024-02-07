use chrono::{Timelike, Utc};
use graphile_worker::helpers::RescheduleJobOptions;

use crate::helpers::{with_test_db, SelectionOfJobs};

mod helpers;

#[tokio::test]
async fn reschedules_jobs_leaves_others_unaffected() {
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

        // Prepare job IDs for rescheduling
        let job_ids_to_reschedule = vec![
            *failed_job.id(),
            *regular_job_1.id(),
            *locked_job.id(), // Note: rescheduling a locked job might have different behavior
            *regular_job_2.id(),
        ];

        // Define a new scheduled time for the jobs
        let nowish = Utc::now().with_nanosecond(0).unwrap() + chrono::Duration::seconds(60);

        // Reschedule specified jobs
        let rescheduled_jobs = worker_utils
            .reschedule_jobs(
                &job_ids_to_reschedule,
                RescheduleJobOptions {
                    run_at: Some(nowish),
                    attempts: Some(1),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to reschedule jobs");

        // Verify the rescheduling of correct jobs and their attributes
        assert_eq!(
            rescheduled_jobs.len(),
            3, // Excluding the locked job
            "Should have rescheduled the specified jobs excluding the locked one"
        );

        for rescheduled_job in &rescheduled_jobs {
            assert_eq!(
                rescheduled_job.last_error().as_deref(),
                if *rescheduled_job.id() == *failed_job.id() {
                    Some("Failed forever")
                } else {
                    None
                },
                "Rescheduled job should have the correct last_error"
            );
            assert_eq!(
                rescheduled_job.attempts(),
                &1,
                "Rescheduled job attempts should be set to 1"
            );
            assert_eq!(
                rescheduled_job.run_at(),
                &nowish,
                "Rescheduled job run_at should be close to nowish"
            );
        }

        // Fetch remaining jobs
        let rescheduled_job_ids: Vec<i64> = rescheduled_jobs.iter().map(|job| *job.id()).collect();
        let remaining_jobs = test_db
            .get_jobs()
            .await
            .into_iter()
            .filter(|job| !rescheduled_job_ids.contains(&job.id))
            .collect::<Vec<_>>();

        // Verify remaining jobs are as expected
        assert_eq!(remaining_jobs.len(), 2, "There should be 2 remaining jobs");
        assert!(
            remaining_jobs.iter().any(|job| job.id == *locked_job.id())
                && remaining_jobs
                    .iter()
                    .any(|job| job.id == *untouched_job.id()),
            "Remaining jobs should include the locked and untouched jobs"
        );
    })
    .await;
}
