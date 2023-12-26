use archimedes_crontab_types::{Crontab, JobKeyMode};
use chrono::prelude::*;
use getset::Getters;
use serde::Serialize;
use serde_json::json;
use sqlx::{query, query_as, FromRow, PgExecutor};
use thiserror::Error;

#[derive(FromRow, Debug, Getters)]
#[getset(get = "pub")]
pub struct KnownCrontab {
    identifier: String,
    known_since: DateTime<Local>,
    last_execution: Option<DateTime<Local>>,
}

pub async fn get_known_crontabs<'e>(
    executor: impl PgExecutor<'e>,
    escaped_schema: &str,
) -> Result<Vec<KnownCrontab>, sqlx::Error> {
    let sql = format!(
        r#"
            select * from {escaped_schema}.known_crontabs
        "#
    );

    let known_crontabs = query_as(&sql).fetch_all(executor).await?;

    Ok(known_crontabs)
}

pub async fn insert_unknown_crontabs<'e, Tz: TimeZone, S: AsRef<str>>(
    executor: impl PgExecutor<'e>,
    escaped_schema: &str,
    unknown_identifiers: &[S],
    start_time: &DateTime<Tz>,
) -> Result<(), sqlx::Error>
where
    Tz::Offset: Send + Sync,
{
    let sql = format!(
        r#"
            INSERT INTO {escaped_schema}.known_crontabs (identifier, known_since)
            SELECT identifier, $2
            FROM unnest($1::text[]) AS unnest (identifier)
            ON CONFLICT DO NOTHING
        "#
    );

    let unknown_identifiers: Vec<&str> = unknown_identifiers.iter().map(|s| s.as_ref()).collect();

    query(&sql)
        .bind(unknown_identifiers)
        .bind(start_time)
        .execute(executor)
        .await?;

    Ok(())
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrontabJobInner {
    task: String,
    payload: Option<serde_json::Value>,
    queue_name: Option<String>,
    run_at: DateTime<Local>,
    max_attempts: Option<u16>,
    priority: Option<i16>,
    job_key: Option<String>,
    job_key_mode: Option<JobKeyMode>,
}

impl CrontabJobInner {
    pub fn from_crontab_and_run_at<Tz: TimeZone>(crontab: &Crontab, run_at: &DateTime<Tz>) -> Self {
        Self {
            task: crontab.task_identifier.to_owned(),
            payload: crontab.payload.to_owned(),
            queue_name: crontab.options.queue.to_owned(),
            run_at: run_at.with_timezone(&Local),
            max_attempts: crontab.options.max.to_owned(),
            priority: crontab.options.priority.to_owned(),
            job_key: crontab.options.job_key.to_owned(),
            job_key_mode: crontab.options.job_key_mode.to_owned(),
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrontabJob {
    identifier: String,
    job: CrontabJobInner,
}

#[derive(Error, Debug)]
pub enum ScheduleCronJobError {
    #[error("An sql error occured while scheduling cron job : {0}")]
    QueryError(#[from] sqlx::Error),
    #[error("A JSON serialization error occured while scheduling cron job : {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub async fn schedule_cron_jobs<'e, Tz: TimeZone>(
    executor: impl PgExecutor<'e>,
    crontab_jobs: &[CrontabJob],
    last_execution: &DateTime<Tz>,
    escaped_schema: &str,
    use_local_time: bool,
) -> Result<(), ScheduleCronJobError>
where
    Tz::Offset: Send + Sync,
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
                    ((json->'job')->>'priority')::smallint as priority,
                    ((json->'job')->>'jobKey')::text as job_key,
                    ((json->'job')->>'jobKeyMode')::text as job_key_mode
                from json_array_elements($1::json) with ordinality AS entries (json, index)
            ),
            locks as (
                insert into {escaped_schema}.known_crontabs as known_crontabs (identifier, known_since, last_execution)
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
                {escaped_schema}.add_job(
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

    query(&statement)
        .bind(serde_json::to_string(crontab_jobs)?)
        .bind(last_execution)
        .bind(use_local_time.then(Local::now))
        .execute(executor)
        .await?;

    Ok(())
}

impl CrontabJob {
    pub fn for_cron<Tz: TimeZone>(crontab: &Crontab, ts: &DateTime<Tz>, backfilled: bool) -> Self {
        let mut job = CrontabJobInner::from_crontab_and_run_at(crontab, ts);

        if let Some(payload) = job.payload.as_mut().and_then(|p| p.as_object_mut()) {
            payload.insert(
                "_cron".into(),
                json!({
                    "ts": format!("{ts:?}"),
                    "backfilled": backfilled
                }),
            );
        }

        Self {
            identifier: crontab.identifier().to_owned(),
            job,
        }
    }
}
