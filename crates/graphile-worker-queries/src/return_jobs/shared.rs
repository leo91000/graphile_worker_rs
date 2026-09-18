use std::time::Duration;

use graphile_worker_database::DbValue;
use graphile_worker_job::Job;

use crate::duration::duration_as_millis_i64;
pub(super) fn recovery_params(
    worker_id: &str,
    job: &Job,
    recovery_delay: Option<Duration>,
    last_error: Option<&str>,
) -> Vec<DbValue> {
    vec![
        DbValue::Text(worker_id.to_string()),
        DbValue::I64(*job.id()),
        DbValue::I64Opt(recovery_delay.map(duration_as_millis_i64)),
        DbValue::TextOpt(last_error.map(ToString::to_string)),
    ]
}
