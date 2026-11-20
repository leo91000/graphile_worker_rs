use chrono::prelude::*;
use graphile_worker_database::{DbError, DbExecutorArg, DbValue, Schema};
use indoc::formatdoc;
use thiserror::Error;

use super::job::CrontabJob;

#[derive(Error, Debug)]
pub enum ScheduleCronJobError {
    #[error("An sql error occurred while scheduling cron job : {0}")]
    QueryError(#[from] DbError),
    #[error("A JSON serialization error occurred while scheduling cron job : {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub(crate) async fn schedule_cron_jobs<Tz: TimeZone>(
    mut executor: impl DbExecutorArg,
    crontab_jobs: &[CrontabJob],
    last_execution: &DateTime<Tz>,
    schema: &Schema,
    use_local_time: bool,
) -> Result<(), ScheduleCronJobError>
where
    Tz::Offset: Send + Sync,
{
    let known_crontabs = schema.private_table("known_crontabs");
    let add_job = schema.function("add_job");
    let statement = formatdoc!(
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
                    ((json->'job')->>'priority')::smallint as priority,
                    ((json->'job')->>'jobKey')::text as job_key,
                    ((json->'job')->>'jobKeyMode')::text as job_key_mode
                from json_array_elements($1::json) with ordinality AS entries (json, index)
            ),
            locks as (
                insert into {known_crontabs} as known_crontabs (identifier, known_since, last_execution)
                select
                    specs.identifier,
                    $2::timestamptz as known_since,
                    $2::timestamptz as last_execution
                from specs
                on conflict (identifier)
                do update set last_execution = excluded.last_execution
                where (known_crontabs.last_execution is null or known_crontabs.last_execution < excluded.last_execution)
                returning known_crontabs.identifier
            )
            select
                {add_job}(
                    specs.task,
                    specs.payload,
                    specs.queue_name,
                    coalesce(specs.run_at, $3::timestamptz, now()),
                    specs.max_attempts,
                    specs.job_key,
                    specs.priority,
                    null, -- flags
                    specs.job_key_mode
                )
            from specs
            inner join locks on (locks.identifier = specs.identifier)
            order by specs.index asc
        "#
    );

    let app_time = use_local_time.then(Utc::now);

    executor
        .execute(
            &statement,
            vec![
                DbValue::Json(serde_json::to_value(crontab_jobs)?),
                DbValue::TimestampTz(last_execution.with_timezone(&Utc)),
                DbValue::TimestampTzOpt(app_time),
            ]
            .into(),
        )
        .await?;

    Ok(())
}
