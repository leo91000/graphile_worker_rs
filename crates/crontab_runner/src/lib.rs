use std::fmt::Display;

use chrono::prelude::*;
use crontab_types::Crontab;
use serde::Serialize;
use sqlx::{query, PgExecutor};
use thiserror::Error;

mod backfill;
mod sql;

impl From<&Crontab> for CrontabJob {
    fn from(crontab: &Crontab) -> Self {
        CrontabJob {
            task: crontab.task_identifier.clone(),
            payload: crontab.payload.clone(),
            queue_name: crontab.options.queue.clone(),
            run_at: Local::now(),
            max_attempts: crontab.options.max,
            priority: crontab.options.priority,
        }
    }
}

async fn schedule_crontab_jobs_at<'e, Tz: TimeZone, C: AsRef<Crontab>>(
    crontabs: &[C],
    executor: impl PgExecutor<'e>,
    at: DateTime<Tz>,
    escaped_schema: &str,
    use_local_time: bool,
) -> Result<(), ScheduleCronJobError>
where
    Tz::Offset: Display,
{
    let jobs: Vec<CrontabJob> = crontabs
        .iter()
        .filter(|crontab| crontab.as_ref().timer().should_run_at(&at.naive_local()))
        .map(|c| c.as_ref().into())
        .collect();

    schedule_cron_jobs(
        executor,
        &jobs,
        &Local::now(),
        escaped_schema,
        use_local_time,
    )
    .await?;

    Ok(())
}
