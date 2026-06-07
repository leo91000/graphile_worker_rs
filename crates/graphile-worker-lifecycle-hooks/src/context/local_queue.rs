use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalQueueMode {
    Starting,
    Polling,
    Waiting,
    TtlExpired,
    Released,
}

#[derive(Debug, Clone)]
pub struct LocalQueueInitContext {
    pub worker_id: String,
}

#[derive(Debug, Clone)]
pub struct LocalQueueSetModeContext {
    pub worker_id: String,
    pub old_mode: LocalQueueMode,
    pub new_mode: LocalQueueMode,
}

#[derive(Debug, Clone)]
pub struct LocalQueueGetJobsCompleteContext {
    pub worker_id: String,
    pub jobs_count: usize,
}

#[derive(Debug, Clone)]
pub struct LocalQueueReturnJobsContext {
    pub worker_id: String,
    pub jobs_count: usize,
}

#[derive(Debug, Clone)]
pub struct LocalQueueRefetchDelayStartContext {
    pub worker_id: String,
    pub duration: Duration,
    pub threshold: usize,
    pub abort_threshold: usize,
}

#[derive(Debug, Clone)]
pub struct LocalQueueRefetchDelayAbortContext {
    pub worker_id: String,
    pub count: usize,
    pub abort_threshold: usize,
}

#[derive(Debug, Clone)]
pub struct LocalQueueRefetchDelayExpiredContext {
    pub worker_id: String,
}
