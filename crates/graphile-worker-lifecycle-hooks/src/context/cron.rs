use graphile_worker_crontab_types::Crontab;
use graphile_worker_job_spec::JobSpec;
use time::OffsetDateTime;

#[derive(Clone)]
pub struct CronTickContext {
    pub timestamp: OffsetDateTime,
    pub crontabs: Vec<Crontab>,
}

#[derive(Clone)]
pub struct CronJobScheduledContext {
    pub crontab: Crontab,
    pub scheduled_at: OffsetDateTime,
}

/// A batch of cron occurrences selected for one historical minute, before SQL
/// attempts to schedule them. Another scheduler may have already claimed them.
#[derive(Clone)]
pub struct CronBackfillContext {
    pub scheduled_at: OffsetDateTime,
    pub entries: Vec<CronBackfillEntry>,
}

/// A selected cron entry and the persisted history used to select it.
#[derive(Clone)]
pub struct CronBackfillEntry {
    pub crontab: Crontab,
    pub known_since: OffsetDateTime,
    pub last_execution: Option<OffsetDateTime>,
}

#[derive(Clone)]
pub struct BeforeJobScheduleContext {
    pub identifier: String,
    pub payload: serde_json::Value,
    pub spec: JobSpec,
}
