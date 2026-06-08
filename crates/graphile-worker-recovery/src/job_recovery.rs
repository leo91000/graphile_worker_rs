use chrono::{DateTime, Utc};
use graphile_worker_database::{DbExecutorArg, DbParams, DbValue, Schema};
use graphile_worker_lifecycle_hooks::{JobRecoveryContext, JobRecoveryResult};
use indoc::formatdoc;

use graphile_worker_queries::errors::GraphileWorkerError;
use graphile_worker_queries::fail_job::single::fail_job;
use graphile_worker_queries::return_jobs::recovery::return_job_for_recovery;

use super::types::{JobRecoveryOutcome, JobRecoveryRequest};

const RECOVERY_LAST_ERROR: &str = "Job recovered after worker interruption";

pub async fn apply_job_recovery(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    request: JobRecoveryRequest<'_>,
) -> Result<JobRecoveryOutcome, GraphileWorkerError> {
    let action = match request.hooks {
        Some(hooks) if !hooks.is_empty() => {
            hooks
                .intercept(JobRecoveryContext {
                    job: request.job.clone(),
                    worker_id: request.worker_id.to_string(),
                    previous_worker_id: request.previous_worker_id.to_string(),
                    reason: request.reason,
                })
                .await
        }
        _ => JobRecoveryResult::Default,
    };

    match action {
        JobRecoveryResult::Default => {
            return_job_for_recovery(
                &mut executor,
                &request.job,
                schema,
                request.previous_worker_id,
                Some(request.recovery_delay),
                Some(RECOVERY_LAST_ERROR),
            )
            .await?;
            Ok(JobRecoveryOutcome::Recovered)
        }
        JobRecoveryResult::Reschedule { run_at, attempts } => {
            return_job_for_recovery(
                &mut executor,
                &request.job,
                schema,
                request.previous_worker_id,
                None,
                Some(RECOVERY_LAST_ERROR),
            )
            .await?;
            set_recovered_job_schedule(&mut executor, schema, *request.job.id(), run_at, attempts)
                .await?;
            Ok(JobRecoveryOutcome::Recovered)
        }
        JobRecoveryResult::FailWithBackoff => {
            fail_job(
                &mut executor,
                &request.job,
                schema,
                request.previous_worker_id,
                &format!("{:?}", request.reason),
                None,
            )
            .await?;
            Ok(JobRecoveryOutcome::FailedWithBackoff)
        }
        JobRecoveryResult::Skip => Ok(JobRecoveryOutcome::Skipped),
    }
}

async fn set_recovered_job_schedule(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    job_id: i64,
    run_at: DateTime<Utc>,
    attempts: Option<i16>,
) -> Result<(), GraphileWorkerError> {
    let jobs = schema.private_table("jobs");
    let sql = formatdoc!(
        r#"
            UPDATE {jobs} AS jobs
            SET
                run_at = $2::timestamptz,
                attempts = COALESCE($3::int, jobs.attempts),
                updated_at = now()
            WHERE id = $1::bigint;
        "#
    );

    executor
        .execute(
            &sql,
            DbParams::from(vec![
                DbValue::I64(job_id),
                DbValue::TimestampTz(run_at),
                DbValue::I32Opt(attempts.map(i32::from)),
            ]),
        )
        .await?;

    Ok(())
}
