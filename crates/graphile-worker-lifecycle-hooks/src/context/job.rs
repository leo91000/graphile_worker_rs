use std::sync::Arc;
use std::time::Duration;

use graphile_worker_job::Job;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason {
    TaskError,
    TaskPanic,
    ShutdownAborted,
    WorkerCrashed,
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
pub struct JobInterruptedContext {
    pub job: Arc<Job>,
    pub worker_id: String,
    pub reason: FailureReason,
}

#[derive(Clone)]
pub struct JobRecoveryContext {
    pub job: Arc<Job>,
    pub worker_id: String,
    pub previous_worker_id: String,
    pub reason: FailureReason,
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
