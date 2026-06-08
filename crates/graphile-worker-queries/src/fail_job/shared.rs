use graphile_worker_database::{DbValue, Schema};

use crate::schema_names::PrivateTable;
use graphile_worker_job::Job;

pub(super) struct FailJobTables {
    pub jobs: String,
    pub job_queues: String,
}

impl FailJobTables {
    pub fn new(schema: &Schema) -> Self {
        Self {
            jobs: PrivateTable::Jobs.qualified(schema),
            job_queues: PrivateTable::JobQueues.qualified(schema),
        }
    }
}

pub(super) fn single_job_params(
    job: &Job,
    worker_id: &str,
    message: &str,
    replacement_payload: Option<serde_json::Value>,
) -> Vec<DbValue> {
    vec![
        DbValue::I64(*job.id()),
        DbValue::Text(message.to_string()),
        DbValue::Text(worker_id.to_string()),
        DbValue::JsonOpt(replacement_payload),
    ]
}
