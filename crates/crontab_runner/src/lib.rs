use std::cmp::Ordering;

use archimedes_crontab_types::Crontab;
use backfill::register_and_backfill_items;
use chrono::prelude::*;
use sqlx::PgExecutor;
use tracing::{debug, warn};

pub use crate::sql::ScheduleCronJobError;
use crate::{
    sql::{schedule_cron_jobs, CrontabJob},
    utils::{round_date_minute, sleep_until, ONE_MINUTE},
};

mod backfill;
mod sql;
mod utils;

pub async fn cron_main<'e>(
    executor: impl PgExecutor<'e> + Clone,
    escaped_schema: &str,
    crontabs: &[Crontab],
    use_local_time: bool,
) -> Result<(), ScheduleCronJobError> {
    let start = Local::now();
    debug!(start = ?start, "cron:starting");

    register_and_backfill_items(
        executor.clone(),
        escaped_schema,
        crontabs,
        &start,
        use_local_time,
    )
    .await?;
    debug!(start = ?start, "cron:started");

    let mut ts = round_date_minute(start, true);

    loop {
        sleep_until(ts).await;
        let current_ts = round_date_minute(Local::now(), false);
        let ts_delta = current_ts - ts;

        match ts_delta.num_minutes().cmp(&0) {
            Ordering::Greater => {
                warn!(
                    "Cron fired {}s too early (clock skew?); rescheduling",
                    ts_delta.num_seconds()
                );
                continue;
            }
            Ordering::Less => {
                warn!(
                    "Cron fired too late; catching up {}m{}s behind",
                    ts_delta.num_minutes(),
                    ts_delta.num_seconds() % 60
                );
            }
            _ => {}
        }

        let mut jobs: Vec<CrontabJob> = vec![];

        // Gather relevant jobs
        for cron in crontabs {
            if cron.should_run_at(&ts.naive_local()) {
                jobs.push(CrontabJob::for_cron(cron, &ts, false));
            }
        }

        if !jobs.is_empty() {
            debug!(nb_jobs = jobs.len(), at = ?ts, "cron:schedule");
            schedule_cron_jobs(executor.clone(), &jobs, &ts, escaped_schema, use_local_time)
                .await?;
            debug!(nb_jobs = jobs.len(), at = ?ts, "cron:scheduled");
        }

        ts += *ONE_MINUTE;
    }
}
