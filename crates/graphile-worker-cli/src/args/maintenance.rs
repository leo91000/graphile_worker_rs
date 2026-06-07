use std::time::Duration;

use clap::{Args, ValueEnum};
use graphile_worker::worker_utils::types::CleanupTask;
use serde::Serialize;

use crate::parsers::parse_duration;

#[derive(Args, Debug)]
pub(crate) struct CleanupArgs {
    /// Cleanup tasks to run. Defaults to all tasks when omitted.
    #[arg(value_enum)]
    pub(crate) tasks: Vec<CleanupTaskArg>,
}

#[derive(Args, Debug)]
pub(crate) struct ForceUnlockArgs {
    /// Worker ids to unlock.
    #[arg(required = true)]
    pub(crate) worker_ids: Vec<String>,
}

#[derive(Args, Debug)]
pub(crate) struct SweepStaleWorkersArgs {
    /// Time since last heartbeat before a worker is deemed inactive (e.g. 5m, 300s).
    #[arg(long, value_parser = parse_duration)]
    pub(crate) sweep_threshold: Option<Duration>,

    /// Delay before recovered jobs are eligible to run again (e.g. 30s).
    #[arg(long, value_parser = parse_duration)]
    pub(crate) recovery_delay: Option<Duration>,

    /// List stale workers without recovering jobs.
    #[arg(long)]
    pub(crate) dry_run: bool,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
pub(crate) enum CleanupTaskArg {
    DeletePermanentlyFailedJobs,
    GcTaskIdentifiers,
    GcJobQueues,
}

impl CleanupTaskArg {
    pub(crate) fn all() -> Vec<Self> {
        vec![
            Self::DeletePermanentlyFailedJobs,
            Self::GcTaskIdentifiers,
            Self::GcJobQueues,
        ]
    }
}

impl From<CleanupTaskArg> for CleanupTask {
    fn from(value: CleanupTaskArg) -> Self {
        match value {
            CleanupTaskArg::DeletePermanentlyFailedJobs => CleanupTask::DeletePermanentlyFailedJobs,
            CleanupTaskArg::GcTaskIdentifiers => CleanupTask::GcTaskIdentifiers,
            CleanupTaskArg::GcJobQueues => CleanupTask::GcJobQueues,
        }
    }
}
