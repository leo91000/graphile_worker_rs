use chrono::prelude::*;
use graphile_worker_crontab_types::{Crontab, JobKeyMode};
use serde::Serialize;
use serde_json::json;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CrontabJobInner {
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
    fn from_crontab_and_run_at<Tz: TimeZone>(crontab: &Crontab, run_at: &DateTime<Tz>) -> Self {
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
pub(crate) struct CrontabJob {
    identifier: String,
    job: CrontabJobInner,
}

impl CrontabJob {
    pub(crate) fn for_cron<Tz: TimeZone>(
        crontab: &Crontab,
        ts: &DateTime<Tz>,
        backfilled: bool,
    ) -> Self {
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
