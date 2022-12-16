use chrono::prelude::*;
use crontab_types::Crontab;
use sqlx::PgExecutor;

async fn schedule_crontab_jobs_at<'e>(
    crontab: &Crontab,
    executor: impl PgExecutor<'e>,
    at: DateTime<Utc>,
) {
}
