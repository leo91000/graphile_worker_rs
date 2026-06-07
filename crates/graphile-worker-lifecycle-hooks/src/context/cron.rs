use chrono::{DateTime, Utc};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_job_spec::JobSpec;

#[derive(Clone)]
pub struct CronTickContext {
    pub timestamp: DateTime<Utc>,
    pub crontabs: Vec<Crontab>,
}

#[derive(Clone)]
pub struct CronJobScheduledContext {
    pub crontab: Crontab,
    pub scheduled_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct BeforeJobScheduleContext {
    pub identifier: String,
    pub payload: serde_json::Value,
    pub spec: JobSpec,
}
