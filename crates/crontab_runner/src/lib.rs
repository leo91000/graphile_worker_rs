use std::fmt::Display;

use chrono::prelude::*;
use crontab_types::Crontab;
use serde::Serialize;
use sqlx::{query, PgExecutor};
use thiserror::Error;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrontabJob {
    task: String,
    payload: Option<serde_json::Value>,
    queue_name: Option<String>,
    run_at: DateTime<Local>,
    max_attempts: Option<u16>,
    priority: Option<i16>,
}

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

#[derive(Error, Debug)]
enum ScheduleCronJobError {
    #[error("An sql error occured while scheduling cron job : {0}")]
    QueryError(#[from] sqlx::Error),
    #[error("A JSON serialization error occured while scheduling cron job : {0}")]
    SerializationError(#[from] serde_json::Error),
}

async fn schedule_cron_jobs<'e, Tz: TimeZone>(
    executor: impl PgExecutor<'e>,
    crontab_jobs: &[CrontabJob],
    last_execution: &DateTime<Tz>,
    escaped_schema: &str,
    use_local_time: bool,
) -> Result<(), ScheduleCronJobError>
where
    Tz::Offset: Display,
{
    let statement = format!(
        r#"
            with specs as (
                select
                    index,
                    (json->>'identifier')::text as identifier,
                    ((json->'job')->>'task')::text as task,
                    ((json->'job')->'payload')::json as payload,
                    ((json->'job')->>'queueName')::text as queue_name,
                    ((json->'job')->>'runAt')::timestamptz as run_at,
                    ((json->'job')->>'maxAttempts')::smallint as max_attempts,
                    ((json->'job')->>'priority')::smallint as priority
                from json_array_elements($1::json) with ordinality AS entries (json, index)
            ),
            locks as (
                insert into {escaped_schema}.known_crontabs (identifier, known_since, last_execution)
                    select
                        specs.identifier,
                        $2 as known_since,
                        $2 as last_execution
                    from specs
                on conflict (identifier)
                do update set last_execution = excluded.last_execution
                where (known_crontabs.last_execution is null or known_crontabs.last_execution < excluded.last_execution)
                returning known_crontabs.identifier
            )
            select
                {escaped_schema}.add_job(
                    specs.task,
                    specs.payload,
                    specs.queue_name,
                    coalesce(specs.run_at, $3::timestamptz, now()),
                    specs.max_attempts,
                    null, -- job key
                    specs.priority
                )
            from specs
            inner join locks on (locks.identifier = specs.identifier)
            order by specs.index asc
        "#
    );

    query(&statement)
        .bind(serde_json::to_string(crontab_jobs)?)
        .bind(format!("{}", last_execution.format("%+")))
        .bind(use_local_time.then(|| Local::now()))
        .execute(executor)
        .await?;

    Ok(())
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

fn round_date_minute<Tz: TimeZone>(mut datetime: DateTime<Tz>, round_up: bool) -> DateTime<Tz> {
    datetime = datetime.with_second(0).unwrap();
    if round_up {
        datetime += chrono::Duration::minutes(1);
    }
    datetime
}

async fn backfill_crontab_jobs<'e>(
    crontabs: &[Crontab],
    executor: impl PgExecutor<'e>,
    escaped_schema: &str,
    use_local_time: bool,
) -> Result<(), ScheduleCronJobError> {
    let mut crontabs_with_backfill = crontabs
        .iter()
        .filter(|c| c.options().fill().is_some())
        .collect::<Vec<_>>();

    if crontabs_with_backfill.len() > 0 {
        let start_time = Local::now();

        // Sort biggest fill first
        crontabs_with_backfill.sort_unstable_by(|a, b| {
            let fill_a = a.options().fill();
            let fill_b = b.options().fill();

            match (fill_a, fill_b) {
                (Some(fill_a), Some(fill_b)) => fill_a.cmp(fill_b),
                _ => unreachable!("should not happen because we have filtered out none fill"),
            }
        });

        let largest_backfill = crontabs_with_backfill[0].options().fill().as_ref().unwrap();
        let largest_backfill_dur = chrono::Duration::seconds(largest_backfill.to_secs() as i64);
        let mut ts = round_date_minute(start_time - largest_backfill_dur, true);

        let one_minute = chrono::Duration::minutes(1);
        while ts < start_time {
            schedule_crontab_jobs_at(
                crontabs_with_backfill,
                executor,
                ts,
                escaped_schema,
                use_local_time,
            )
            .await?;
            ts += one_minute;
        }
    }

    Ok(())
}
