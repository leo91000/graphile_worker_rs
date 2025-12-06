use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_extensions::ReadOnlyExtensions;
use graphile_worker_job::Job;
use graphile_worker_job_spec::JobSpec;
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    Signal,
    Error,
    Graceful,
}

#[derive(Clone)]
pub struct WorkerInitContext {
    pub pool: PgPool,
    pub schema: String,
    pub concurrency: usize,
}

#[derive(Clone)]
pub struct WorkerStartContext {
    pub pool: PgPool,
    pub worker_id: String,
    pub extensions: ReadOnlyExtensions,
}

#[derive(Clone)]
pub struct WorkerShutdownContext {
    pub pool: PgPool,
    pub worker_id: String,
    pub reason: ShutdownReason,
}

#[derive(Clone)]
pub struct JobFetchContext {
    pub job: Arc<Job>,
    pub worker_id: String,
}

#[derive(Clone)]
pub struct JobStartContext {
    pub job: Arc<Job>,
    pub worker_id: String,
}

#[derive(Clone)]
pub struct JobCompleteContext {
    pub job: Arc<Job>,
    pub worker_id: String,
    pub duration: Duration,
}

#[derive(Clone)]
pub struct JobFailContext {
    pub job: Arc<Job>,
    pub worker_id: String,
    pub error: String,
    pub will_retry: bool,
}

#[derive(Clone)]
pub struct JobPermanentlyFailContext {
    pub job: Arc<Job>,
    pub worker_id: String,
    pub error: String,
}

#[derive(Clone)]
pub struct BeforeJobRunContext {
    pub job: Arc<Job>,
    pub worker_id: String,
    pub payload: serde_json::Value,
}

#[derive(Clone)]
pub struct AfterJobRunContext {
    pub job: Arc<Job>,
    pub worker_id: String,
    pub result: Result<(), String>,
    pub duration: Duration,
}

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
