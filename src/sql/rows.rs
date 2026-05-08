use chrono::{DateTime, Utc};
use graphile_worker_database::{DbError, DbRow};
use graphile_worker_job::DbJob;

pub fn db_job_from_row(row: &DbRow) -> Result<DbJob, DbError> {
    Ok(DbJob::new(
        row.try_get("id")?,
        row.try_get("job_queue_id")?,
        row.try_get("payload")?,
        row.try_get("priority")?,
        row.try_get::<DateTime<Utc>>("run_at")?,
        row.try_get("attempts")?,
        row.try_get("max_attempts")?,
        row.try_get("last_error")?,
        row.try_get::<DateTime<Utc>>("created_at")?,
        row.try_get::<DateTime<Utc>>("updated_at")?,
        row.try_get("key")?,
        row.try_get("revision")?,
        row.try_get("locked_at")?,
        row.try_get("locked_by")?,
        row.try_get("flags")?,
        row.try_get("task_id")?,
    ))
}
