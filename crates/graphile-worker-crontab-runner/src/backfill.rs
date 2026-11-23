use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::{DbExecutorArg, Schema};
use graphile_worker_lifecycle_hooks::{CronBackfillContext, CronBackfillEntry, HookRegistry};
use time::{Duration, OffsetDateTime, UtcOffset};
use tracing::debug;

use crate::{
    clock::Clock,
    runner::CronEntries,
    sql::{
        get_known_crontabs, insert_unknown_crontabs, schedule_cron_jobs, CrontabJob, KnownCrontab,
        ScheduleCronJobError,
    },
    utils::{round_date_minute, yield_now, ONE_MINUTE},
};

pub(crate) struct BackfillItemAndDate<'a, 'b> {
    index: usize,
    item: &'a Crontab,
    known: &'b KnownCrontab,
}

pub(crate) struct BackfillAndUnknownItems<'a, 'b> {
    backfill_items_and_date: Vec<BackfillItemAndDate<'a, 'b>>,
    unknown_identifiers: Vec<&'a str>,
}

pub(crate) fn get_backfill_and_unknown_items<'a, 'b>(
    crontabs: &'a [Crontab],
    known_crontabs: &'b [KnownCrontab],
) -> BackfillAndUnknownItems<'a, 'b> {
    let mut backfill_items_and_date = vec![];
    let mut unknown_identifiers = vec![];

    for (index, crontab) in crontabs.iter().enumerate() {
        let known = known_crontabs
            .iter()
            .find(|uc| uc.identifier() == crontab.identifier());

        if let Some(known) = known {
            backfill_items_and_date.push(BackfillItemAndDate {
                index,
                item: crontab,
                known,
            });
        } else {
            unknown_identifiers.push(crontab.identifier());
        }
    }

    BackfillAndUnknownItems {
        backfill_items_and_date,
        unknown_identifiers,
    }
}

pub(crate) async fn register_and_backfill_items(
    mut executor: impl DbExecutorArg,
    schema: &Schema,
    entries: CronEntries<'_>,
    start_time: OffsetDateTime,
    clock: &impl Clock,
    use_local_time: bool,
    hooks: &HookRegistry,
) -> Result<(), ScheduleCronJobError> {
    let known_crontabs = get_known_crontabs(&mut executor, schema).await?;

    let BackfillAndUnknownItems {
        backfill_items_and_date,
        unknown_identifiers,
    } = get_backfill_and_unknown_items(entries.crontabs, &known_crontabs);

    if !unknown_identifiers.is_empty() {
        insert_unknown_crontabs(&mut executor, schema, &unknown_identifiers, start_time).await?;
    }

    let largest_backfill = backfill_items_and_date
        .iter()
        .filter_map(|c| c.item.options().fill().as_ref())
        .map(|f| f.to_secs())
        .max();

    if let Some(largest_backfill) = largest_backfill {
        let mut ts = round_date_minute(
            start_time - Duration::seconds(largest_backfill as i64),
            true,
        );

        while ts < start_time {
            yield_now().await;
            let time_ago = (start_time - ts).whole_seconds();
            let local_ts = clock.local_time(ts);
            let calendar_time = crate::utils::calendar_time(local_ts, use_local_time);

            let mut candidates = Vec::new();
            for candidate in &backfill_items_and_date {
                let Some(backfill) = candidate.item.options().fill().as_ref() else {
                    continue;
                };
                let not_before = candidate
                    .known
                    .last_execution()
                    .as_ref()
                    .unwrap_or_else(|| candidate.known.known_since());
                if backfill.to_secs() as i64 >= time_ago
                    && &ts >= not_before
                    && entries.matches(candidate.index, &calendar_time)
                {
                    candidates.push(candidate);
                }
            }

            if !candidates.is_empty() {
                debug!(nb_jobs = candidates.len(), at = ?ts, "cron:backfill");
                hooks
                    .emit(CronBackfillContext {
                        scheduled_at: ts.to_offset(UtcOffset::UTC),
                        entries: candidates
                            .iter()
                            .map(|candidate| CronBackfillEntry {
                                crontab: candidate.item.clone(),
                                known_since: candidate
                                    .known
                                    .known_since()
                                    .to_offset(UtcOffset::UTC),
                                last_execution: candidate
                                    .known
                                    .last_execution()
                                    .map(|time| time.to_offset(UtcOffset::UTC)),
                            })
                            .collect(),
                    })
                    .await;
                let to_backfill: Vec<_> = candidates
                    .into_iter()
                    .map(|candidate| CrontabJob::for_cron(candidate.item, local_ts, true))
                    .collect::<Result<_, _>>()?;
                schedule_cron_jobs(&mut executor, &to_backfill, ts, schema, use_local_time).await?;
            }

            ts += ONE_MINUTE;
        }
    }

    Ok(())
}
