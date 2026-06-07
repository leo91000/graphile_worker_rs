use chrono::{DateTime, Utc};
use graphile_worker_database::{DbError, DbRow, FromDbCell};
use graphile_worker_job::{DbJob, DbJobData};

use crate::errors::Result as WorkerResult;

pub fn db_job_from_row(row: &DbRow) -> core::result::Result<DbJob, DbError> {
    Ok(DbJob::from_data(DbJobData {
        id: row.try_get("id")?,
        job_queue_id: row.try_get("job_queue_id")?,
        payload: row.try_get("payload")?,
        priority: row.try_get("priority")?,
        run_at: row.try_get::<DateTime<Utc>>("run_at")?,
        attempts: row.try_get("attempts")?,
        max_attempts: row.try_get("max_attempts")?,
        last_error: row.try_get("last_error")?,
        created_at: row.try_get::<DateTime<Utc>>("created_at")?,
        updated_at: row.try_get::<DateTime<Utc>>("updated_at")?,
        key: row.try_get("key")?,
        revision: row.try_get("revision")?,
        locked_at: row.try_get("locked_at")?,
        locked_by: row.try_get("locked_by")?,
        flags: row.try_get("flags")?,
        task_id: row.try_get("task_id")?,
    }))
}

pub(crate) fn collect_column<T>(rows: &[DbRow], column: &str) -> WorkerResult<Vec<T>>
where
    T: FromDbCell,
{
    rows.iter()
        .map(|row| row.try_get::<T>(column).map_err(Into::into))
        .collect()
}

pub(crate) fn get_required<T>(row: &DbRow, column: &str) -> WorkerResult<T>
where
    T: FromDbCell,
{
    row.try_get(column).map_err(Into::into)
}
