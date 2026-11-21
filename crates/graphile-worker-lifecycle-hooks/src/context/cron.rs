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

/// A batch of cron occurrences selected for one historical minute, before SQL
/// attempts to schedule them. Another scheduler may have already claimed them.
#[derive(Clone)]
pub struct CronBackfillContext {
    pub scheduled_at: DateTime<Utc>,
    pub entries: Vec<CronBackfillEntry>,
}

/// A selected cron entry and the persisted history used to select it.
#[derive(Clone)]
pub struct CronBackfillEntry {
    pub crontab: Crontab,
    pub known_since: DateTime<Utc>,
    pub last_execution: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct BeforeJobScheduleContext {
    pub identifier: String,
    pub payload: serde_json::Value,
    pub spec: JobSpec,
}
