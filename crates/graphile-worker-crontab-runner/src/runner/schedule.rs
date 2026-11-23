use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::{DbExecutorArg, Schema};
use graphile_worker_lifecycle_hooks::{CronJobScheduledContext, CronTickContext, HookRegistry};
use time::{OffsetDateTime, UtcOffset};
use tracing::debug;

use super::CronEntries;

use crate::sql::{schedule_cron_jobs, CrontabJob, ScheduleCronJobError};
use crate::utils::calendar_time;

pub(super) async fn emit_tick_and_schedule_jobs(
    executor: &mut impl DbExecutorArg,
    schema: &Schema,
    entries: CronEntries<'_>,
    use_local_time: bool,
    hooks: &HookRegistry,
    ts: OffsetDateTime,
) -> Result<(), ScheduleCronJobError> {
    emit_tick(hooks, entries.crontabs, ts).await;

    let jobs = collect_jobs_to_schedule(hooks, entries, ts, use_local_time).await?;
    if jobs.is_empty() {
        return Ok(());
    }

    debug!(nb_jobs = jobs.len(), at = ?ts, "cron:schedule");
    schedule_cron_jobs(executor, &jobs, ts, schema, use_local_time).await?;
    debug!(nb_jobs = jobs.len(), at = ?ts, "cron:scheduled");

    Ok(())
}

async fn emit_tick(hooks: &HookRegistry, crontabs: &[Crontab], ts: OffsetDateTime) {
    let tick_ctx = CronTickContext {
        timestamp: ts.to_offset(UtcOffset::UTC),
        crontabs: crontabs.to_vec(),
    };
    hooks.emit(tick_ctx).await;
}

async fn collect_jobs_to_schedule(
    hooks: &HookRegistry,
    entries: CronEntries<'_>,
    ts: OffsetDateTime,
    use_local_time: bool,
) -> Result<Vec<CrontabJob>, ScheduleCronJobError> {
    let mut jobs = Vec::new();
    let calendar_time = calendar_time(ts, use_local_time);

    for (index, cron) in entries.crontabs.iter().enumerate() {
        if entries.matches(index, &calendar_time) {
            jobs.push(CrontabJob::for_cron(cron, ts, false)?);

            let scheduled_ctx = CronJobScheduledContext {
                crontab: cron.clone(),
                scheduled_at: ts.to_offset(UtcOffset::UTC),
            };
            hooks.emit(scheduled_ctx).await;
        }
    }

    Ok(jobs)
}
