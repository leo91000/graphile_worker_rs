use std::time::Duration;

use graphile_worker_database::{DbExecutorArg, DbParams, DbValue};
use indoc::formatdoc;

use crate::errors::Result;

fn duration_to_interval(duration: Duration) -> String {
    format!("{} seconds", duration.as_secs())
}

pub async fn recover_dead_worker_jobs(
    mut executor: impl DbExecutorArg,
    escaped_schema: &str,
    worker_ids: &[String],
    recovery_delay: Duration,
) -> Result<i32> {
    if worker_ids.is_empty() {
        return Ok(0);
    }

    let sql = formatdoc!(
        r#"
            SELECT {escaped_schema}.recover_dead_worker_jobs($1::text[], $2::interval) AS recovered_count;
        "#
    );

    let row = executor
        .fetch_one(
            &sql,
            DbParams::from(vec![
                DbValue::TextArray(worker_ids.to_vec()),
                DbValue::Text(duration_to_interval(recovery_delay)),
            ]),
        )
        .await?;

    row.try_get("recovered_count").map_err(Into::into)
}