use graphile_worker_database::{DbExecutorArg, DbValue, Schema};
use graphile_worker_job::Job;

use crate::errors::Result;

pub async fn return_jobs(
    mut executor: impl DbExecutorArg,
    jobs: &[Job],
    schema: impl Into<Schema>,
    worker_id: &str,
) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }
    let function = schema.into().function("_private_return_jobs");
    executor
        .execute(
            &format!("select {function}($1::text, $2::bigint[])"),
            vec![
                DbValue::Text(worker_id.to_owned()),
                DbValue::I64Array(jobs.iter().map(|job| *job.id()).collect()),
            ]
            .into(),
        )
        .await?;
    Ok(())
}
