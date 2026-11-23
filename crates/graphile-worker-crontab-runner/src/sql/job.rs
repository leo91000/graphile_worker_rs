use graphile_worker_crontab_types::{Crontab, JobKeyMode};
use serde::Serialize;
use time::{OffsetDateTime, UtcOffset};

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
            run_at: run_at.to_offset(UtcOffset::UTC),
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
            let ts = if ts.offset().seconds_past_minute() == 0 {
                ts
            } else {
                ts.to_offset(UtcOffset::UTC)
            };
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

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn serializes_cron_tick_with_subminute_offset() {
        let crontab = Crontab {
            payload: Some(serde_json::json!({})),
            ..Crontab::default()
        };
        let local_tick = datetime!(2026-01-01 07:00:00 +05:30:20);
        let job = CrontabJob::for_cron(&crontab, local_tick, false).unwrap();
        let job = serde_json::to_value(job).unwrap();

        assert_eq!(job["job"]["runAt"], "2026-01-01T01:29:40Z");
        assert_eq!(job["job"]["payload"]["_cron"]["ts"], "2026-01-01T01:29:40Z");
    }
}
