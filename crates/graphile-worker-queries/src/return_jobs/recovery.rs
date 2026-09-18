use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, Schema};
use graphile_worker_job::Job;

use super::shared::recovery_params;
use crate::errors::Result;

/// Releases an owned job after interruption without retry backoff.
///
/// An optional delay moves `run_at` forward, and an optional error replaces the
/// last error. Retired jobs stay exhausted; ordinary claims regain an attempt.
/// The current migrated schema preserves queue ownership until this release.
pub async fn return_job_for_recovery(
    mut executor: impl DbExecutorArg,
    job: &Job,
    schema: impl Into<Schema>,
    worker_id: &str,
    recovery_delay: Option<Duration>,
    last_error: Option<&str>,
) -> Result<()> {
    let function = schema.into().function("_private_return_jobs");
    executor
        .execute(
            &format!("select {function}($1::text, array[$2::bigint], $3::bigint, $4::text, true)"),
            recovery_params(worker_id, job, recovery_delay, last_error).into(),
        )
        .await?;
    Ok(())
}
