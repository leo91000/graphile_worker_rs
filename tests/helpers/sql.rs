use chrono::{DateTime, Local, Utc};
use graphile_worker::{DbJob, DbJobData};
use graphile_worker_crontab_runner::KnownCrontab;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs, QueryScalar};
use sqlx::{FromRow, Postgres, Row};

pub fn safe_query(sql: impl Into<String>) -> Query<'static, Postgres, PgArguments> {
    sqlx::query(sqlx::AssertSqlSafe(sql.into()))
}

pub fn safe_query_as<T>(sql: impl Into<String>) -> QueryAs<'static, Postgres, T, PgArguments>
where
    T: for<'row> FromRow<'row, PgRow>,
{
    sqlx::query_as(sqlx::AssertSqlSafe(sql.into()))
}

pub fn safe_query_scalar<T>(
    sql: impl Into<String>,
) -> QueryScalar<'static, Postgres, T, PgArguments>
where
    (T,): for<'row> FromRow<'row, PgRow>,
{
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.into()))
}

pub(super) fn db_job_from_sqlx_row(row: PgRow) -> DbJob {
    DbJob::from_data(DbJobData {
        id: row.get("id"),
        job_queue_id: row.get("job_queue_id"),
        payload: row.get("payload"),
        priority: row.get("priority"),
        run_at: row.get("run_at"),
        attempts: row.get("attempts"),
        max_attempts: row.get("max_attempts"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        key: row.get("key"),
        revision: row.get("revision"),
        locked_at: row.get("locked_at"),
        locked_by: row.get("locked_by"),
        flags: row.get("flags"),
        task_id: row.get("task_id"),
    })
}

pub(super) fn known_crontab_from_sqlx_row(row: PgRow) -> KnownCrontab {
    let known_since: DateTime<Utc> = row.get("known_since");
    let last_execution: Option<DateTime<Utc>> = row.get("last_execution");

    KnownCrontab::new(
        row.get("identifier"),
        known_since.with_timezone(&Local),
        last_execution.map(|value| value.with_timezone(&Local)),
    )
}
