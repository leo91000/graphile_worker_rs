use graphile_worker_crontab_types::{Crontab, JobKeyMode};
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CrontabJobInner {
    task: String,
    payload: Option<serde_json::Value>,
    queue_name: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    run_at: OffsetDateTime,
    max_attempts: Option<u16>,
    priority: Option<i16>,
    job_key: Option<String>,
    job_key_mode: Option<JobKeyMode>,
}

impl CrontabJobInner {
    fn from_crontab_and_run_at(crontab: &Crontab, run_at: OffsetDateTime) -> Self {
        Self {
            task: crontab.task_identifier.to_owned(),
            payload: crontab.payload.to_owned(),
            queue_name: crontab.options.queue.to_owned(),
            run_at,
            max_attempts: crontab.options.max.to_owned(),
            priority: crontab.options.priority.to_owned(),
            job_key: crontab.options.job_key.to_owned(),
            job_key_mode: crontab.options.job_key_mode.to_owned(),
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrontabJob {
    identifier: String,
    job: CrontabJobInner,
}

#[derive(Serialize)]
struct CronMetadata {
    #[serde(with = "time::serde::rfc3339")]
    ts: OffsetDateTime,
    backfilled: bool,
}

impl CrontabJob {
    pub(crate) fn for_cron(
        crontab: &Crontab,
        ts: OffsetDateTime,
        backfilled: bool,
    ) -> serde_json::Result<Self> {
        let mut job = CrontabJobInner::from_crontab_and_run_at(crontab, ts);

        if let Some(payload) = job.payload.as_mut().and_then(|p| p.as_object_mut()) {
            payload.insert(
                "_cron".into(),
                serde_json::to_value(CronMetadata { ts, backfilled })?,
            );
        }

        Ok(Self {
            identifier: crontab.identifier().to_owned(),
            job,
        })
    }
}
