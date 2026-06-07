use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MaintenanceRequest {
    pub action: MaintenanceAction,
    #[serde(default)]
    pub cleanup_tasks: Vec<CleanupTaskName>,
    #[serde(default)]
    pub worker_ids: Vec<String>,
    #[serde(default)]
    pub dry_run: bool,
    pub sweep_threshold_secs: Option<u64>,
    pub recovery_delay_secs: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaintenanceAction {
    Migrate,
    Cleanup,
    ForceUnlock,
    SweepStaleWorkers,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CleanupTaskName {
    DeletePermanentlyFailedJobs,
    GcTaskIdentifiers,
    GcJobQueues,
}

impl CleanupTaskName {
    pub fn all() -> Vec<Self> {
        vec![
            Self::DeletePermanentlyFailedJobs,
            Self::GcTaskIdentifiers,
            Self::GcJobQueues,
        ]
    }
}
