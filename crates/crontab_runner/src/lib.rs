use chrono::prelude::*;
use crontab_types::Crontab;
use sqlx::PgExecutor;

async fn schedule_crontab_jobs_at(
    crontab: &Crontab,
    executor: impl for<'e> PgExecutor<'e>,
    at: DateTime<Utc>,
) {
}
