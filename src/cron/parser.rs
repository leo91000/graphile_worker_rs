use serde::Deserialize;

pub struct Cron {
    timing: CronTiming,
    task: String,
    options: CronOptions,
    payload: serde_json::Value,
}

pub struct CronTiming {
    minutes: CronTimingType,
    hours: CronTimingType,
    days: CronTimingType,
    months: CronTimingType,
    days_of_week: CronTimingType,
}

pub enum CronTimingType {
    Numbers(Vec<u8>),
    Range(u8, u8),
    Wildcard(Option<u8>),
}

#[derive(Deserialize)]
pub struct CronOptions {
    identifier: Option<String>,
    backfill_period: Option<u8>,
    max_attempts: Option<u8>,
    queue_name: Option<String>,
    priority: Option<u8>,
}
