use chrono::Utc;
use graphile_worker::{Job, JobSpec};

use super::db::TestDatabase;
use super::sql::db_job_from_sqlx_row;

#[derive(Clone)]
pub struct SelectionOfJobs {
    pub failed_job: Job,
    pub regular_job_1: Job,
    pub locked_job: Job,
    pub regular_job_2: Job,
    pub untouched_job: Job,
}

impl TestDatabase {
    pub async fn make_selection_of_jobs(&self) -> SelectionOfJobs {
        let utils = self.worker_utils();

        let in_one_hour = Utc::now() + chrono::Duration::hours(1);
        let failed_job = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 1 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");
        let regular_job_1 = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 2 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");
        let locked_job = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 3 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");
        let regular_job_2 = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 4 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");
        let untouched_job = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 5 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        let locked_job = {
            let locked_job_update = sqlx::query("update graphile_worker._private_jobs as jobs set locked_by = 'test', locked_at = now() where id = $1 returning *")
            .bind(locked_job.id())
            .fetch_one(&self.test_pool)
            .await
            .expect("Failed to lock job");

            Job::from_db_job(db_job_from_sqlx_row(locked_job_update), "job3".to_string())
        };

        let failed_job = {
            let failed_job_update = sqlx::query("update graphile_worker._private_jobs as jobs set last_error = 'Failed forever', attempts = max_attempts where id = $1 returning *")
                .bind(failed_job.id())
                .fetch_one(&self.test_pool)
                .await
                .expect("Failed to fail job");

            Job::from_db_job(db_job_from_sqlx_row(failed_job_update), "job3".to_string())
        };

        SelectionOfJobs {
            failed_job,
            regular_job_1,
            locked_job,
            regular_job_2,
            untouched_job,
        }
    }
}
