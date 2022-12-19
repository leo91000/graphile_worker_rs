use chrono::prelude::*;
use crontab_types::Crontab;
use sqlx::PgExecutor;
use tracing::debug;

use crate::{
    sql::{
        get_known_crontabs, insert_unknown_crontabs, schedule_cron_jobs, CrontabJob, KnownCrontab,
        ScheduleCronJobError,
    },
    utils::{round_date_minute, ONE_MINUTE},
};

pub(crate) struct BackfillItemAndDate<'a, 'b> {
    item: &'a Crontab,
    not_before: &'b DateTime<Local>,
}

pub(crate) struct BackfillAndUnknownItems<'a, 'b> {
    backfill_items_and_date: Vec<BackfillItemAndDate<'a, 'b>>,
    unknown_identifiers: Vec<&'a String>,
}

pub(crate) fn get_backfill_and_unknown_items<'a, 'b>(
    crontabs: &'a [Crontab],
    known_crontabs: &'b [KnownCrontab],
) -> BackfillAndUnknownItems<'a, 'b> {
    let mut backfill_items_and_date = vec![];
    let mut unknown_identifiers = vec![];

    for crontab in crontabs {
        let known = known_crontabs
            .iter()
            .find(|uc| uc.identifier() == crontab.task_identifier());

        if let Some(known) = known {
            let not_before = known
                .last_execution()
                .as_ref()
                .unwrap_or(known.known_since());

            backfill_items_and_date.push(BackfillItemAndDate {
                item: crontab,
                not_before,
            });
        } else {
            unknown_identifiers.push(crontab.task_identifier());
        }
    }

    BackfillAndUnknownItems {
        backfill_items_and_date,
        unknown_identifiers,
    }
}

pub(crate) async fn register_and_backfill_items<'e, Tz: TimeZone>(
    executor: impl PgExecutor<'e> + Clone,
    escaped_schema: &str,
    crontabs: &[Crontab],
    start_time: &DateTime<Tz>,
    use_local_time: bool,
) -> Result<(), ScheduleCronJobError>
where
    Tz::Offset: Send + Sync,
{
    let known_crontabs = get_known_crontabs(executor.clone(), escaped_schema).await?;

    let BackfillAndUnknownItems {
        backfill_items_and_date,
        unknown_identifiers,
    } = get_backfill_and_unknown_items(crontabs, &known_crontabs);

    if unknown_identifiers.len() > 0 {
        insert_unknown_crontabs(
            executor.clone(),
            escaped_schema,
            &unknown_identifiers,
            start_time,
        )
        .await?;
    }

    let largest_backfill = crontabs
        .iter()
        .filter_map(|c| c.options().fill().as_ref().map(|c| c.to_secs()))
        .max();

    if let Some(largest_backfill) = largest_backfill {
        let mut ts = round_date_minute(
            start_time.to_owned() - chrono::Duration::seconds(largest_backfill as i64),
            true,
        );

        while &ts < start_time {
            let time_ago = (start_time.to_owned() - ts.to_owned()).num_seconds();

            let to_backfill: Vec<CrontabJob> = backfill_items_and_date
                .iter()
                .filter_map(|b| {
                    let backfill = b.item.options().fill().as_ref()?;
                    if backfill.to_secs() as i64 >= time_ago
                        && &ts >= b.not_before
                        && b.item.should_run_at(&ts.naive_local())
                    {
                        return Some(CrontabJob::for_cron(b.item, &ts, true));
                    }
                    None
                })
                .collect();

            if to_backfill.len() > 0 {
                debug!(nb_jobs = to_backfill.len(), at = ?ts, "cron:backfill");
                schedule_cron_jobs(
                    executor.clone(),
                    &to_backfill,
                    &ts,
                    escaped_schema,
                    use_local_time,
                )
                .await?;
            }

            ts += *ONE_MINUTE;
        }
    }

    Ok(())
}
