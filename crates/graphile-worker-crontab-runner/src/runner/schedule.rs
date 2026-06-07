use chrono::prelude::*;
use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::{DbExecutorArg, Schema};
use graphile_worker_lifecycle_hooks::{CronJobScheduledContext, CronTickContext, HookRegistry};
use tracing::debug;

use crate::sql::{schedule_cron_jobs, CrontabJob, ScheduleCronJobError};

pub(super) async fn emit_tick_and_schedule_jobs(
    executor: &mut impl DbExecutorArg,
    schema: &Schema,
    crontabs: &[Crontab],
    use_local_time: bool,
    hooks: &HookRegistry,
    ts: DateTime<Local>,
) -> Result<(), ScheduleCronJobError> {
    emit_tick(hooks, crontabs, ts).await;

    let jobs = collect_jobs_to_schedule(hooks, crontabs, ts).await;
    if jobs.is_empty() {
        return Ok(());
    }

    debug!(nb_jobs = jobs.len(), at = ?ts, "cron:schedule");
    schedule_cron_jobs(executor, &jobs, &ts, schema, use_local_time).await?;
    debug!(nb_jobs = jobs.len(), at = ?ts, "cron:scheduled");

    Ok(())
}

async fn emit_tick(hooks: &HookRegistry, crontabs: &[Crontab], ts: DateTime<Local>) {
    let tick_ctx = CronTickContext {
        timestamp: ts.with_timezone(&Utc),
        crontabs: crontabs.to_vec(),
    };
    hooks.emit(tick_ctx).await;
}

async fn collect_jobs_to_schedule(
    hooks: &HookRegistry,
    crontabs: &[Crontab],
    ts: DateTime<Local>,
) -> Vec<CrontabJob> {
    let mut jobs = Vec::new();

    for cron in crontabs {
        if cron.should_run_at(&ts.naive_local()) {
            jobs.push(CrontabJob::for_cron(cron, &ts, false));

            let scheduled_ctx = CronJobScheduledContext {
                crontab: cron.clone(),
                scheduled_at: ts.with_timezone(&Utc),
            };
            hooks.emit(scheduled_ctx).await;
        }
    }

    jobs
}
