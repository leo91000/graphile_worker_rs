use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use graphile_worker_job::Job;
use graphile_worker_lifecycle_hooks::{FailureReason, HookRegistry};
use serde::Serialize;

use super::config::WorkerRecoveryConfig;

#[derive(Debug, Clone, Default)]
pub struct SweepStaleWorkersOptions {
    pub sweep_threshold: Option<Duration>,
    pub recovery_delay: Option<Duration>,
    pub recovery_config: Option<WorkerRecoveryConfig>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SweepStaleWorkersResult {
    pub worker_ids: Vec<String>,
    pub recovered_count: i32,
}

pub(crate) struct JobRecoveryRequest<'a> {
    pub hooks: Option<&'a Arc<HookRegistry>>,
    pub worker_id: &'a str,
    pub job: Arc<Job>,
    pub previous_worker_id: &'a str,
    pub reason: FailureReason,
    pub recovery_delay: Duration,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveWorkerRow {
    pub worker_id: String,
    pub last_heartbeat_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
    pub is_stale: bool,
}
