use std::time::Duration;

use graphile_worker_database::{DbValue, Schema};
use graphile_worker_job::Job;

use crate::duration::duration_as_millis_i64;
use crate::schema_names::PrivateTable;

pub(super) struct ReturnJobTables {
    pub jobs: String,
    pub job_queues: String,
}

impl ReturnJobTables {
    pub fn new(schema: &Schema) -> Self {
        Self {
            jobs: PrivateTable::Jobs.qualified(schema),
            job_queues: PrivateTable::JobQueues.qualified(schema),
        }
    }
}

pub(super) struct ReturnJobIds {
    pub all: Vec<i64>,
    pub queued: Vec<i64>,
}

impl ReturnJobIds {
    pub fn from_jobs(jobs: &[Job]) -> Self {
        let mut all = Vec::with_capacity(jobs.len());
        let mut queued = Vec::new();

        for job in jobs {
            let id = *job.id();
            all.push(id);
            if job.job_queue_id().is_some() {
                queued.push(id);
            }
        }

        Self { all, queued }
    }

    pub fn contains_queued_jobs(&self) -> bool {
        !self.queued.is_empty()
    }
}

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
