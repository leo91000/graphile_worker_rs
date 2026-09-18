use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use graphile_worker_database::{Database, Schema};
pub use graphile_worker_lifecycle_hooks::LocalQueueMode;
use graphile_worker_lifecycle_hooks::{HookRegistry, LocalQueueInitContext};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use thiserror::Error;
use tracing::warn;

use graphile_worker_queries::task_identifiers::SharedTaskDetails;

mod cache;
mod claims;
mod config;
mod fetch;
mod jobs;
mod refetch_delay;
mod release;
mod state;

pub use config::{
    LocalQueueConfig, LocalQueueConfigBuilder, LocalQueueConfigError, RefetchDelayConfig,
    RefetchDelayConfigBuilder,
};
use state::LocalQueueState;

/// Sender for notifying the runner that LocalQueue has cached work available.
pub type LocalQueueSignalSender = runtime::Sender<()>;

/// Receiver for LocalQueue cache notifications.
pub(crate) type LocalQueueSignalReceiver = runtime::Receiver<()>;

#[derive(Debug, Error)]
pub enum LocalQueueError {
    #[error("Failed to return jobs to database: {0}")]
    ReturnJobsError(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::errors::GraphileWorkerError),
}

#[derive(Clone)]
pub struct LocalQueue(Arc<LocalQueueState>);

impl From<LocalQueueState> for LocalQueue {
    fn from(state: LocalQueueState) -> Self {
        Self(Arc::new(state))
    }
}

impl From<LocalQueueParams> for LocalQueue {
    fn from(params: LocalQueueParams) -> Self {
        LocalQueueState::new(params).into()
    }
}

pub struct LocalQueueParams {
    pub config: LocalQueueConfig,
    pub database: Database,
    pub schema: Schema,
    pub worker_id: String,
    pub task_details: SharedTaskDetails,
    pub poll_interval: Duration,
    pub continuous: bool,
    pub shutdown_signal: Option<ShutdownSignal>,
    pub hooks: Arc<HookRegistry>,
    pub job_signal_sender: LocalQueueSignalSender,
    pub use_local_time: bool,
}

impl LocalQueue {
    /// Starts a local fetch loop and, when supplied, its shutdown listener.
    ///
    /// The configuration must be valid for the polling interval. Call `release`
    /// to await cleanup and return cached claims before discarding the queue.
    pub fn new(params: LocalQueueParams) -> Self {
        params
            .config
            .validate(params.poll_interval)
            .expect("invalid local queue config");

        let shutdown_signal = params.shutdown_signal.clone();
        let queue: LocalQueue = params.into();
        queue.0.run_complete.store(false, Ordering::Release);

        let queue_clone = queue.clone();
        let run_task = runtime::spawn(async move {
            queue_clone.run().await;
        });
        queue.0.run_task.replace_abort(run_task);

        if let Some(signal) = shutdown_signal {
            let queue_for_shutdown = queue.clone();
            let shutdown_task = runtime::spawn(async move {
                signal.await;
                if let Err(e) = queue_for_shutdown.release().await {
                    warn!(error = %e, "Error releasing LocalQueue on shutdown");
                }
            });
            queue.0.shutdown_task.replace_abort(shutdown_task);
        }

        queue
    }

    /// Runs fetching and records completion for every waiting release caller.
    async fn run(&self) {
        self.0
            .hooks
            .emit(LocalQueueInitContext {
                worker_id: self.0.worker_id.clone(),
            })
            .await;

        self.set_mode(LocalQueueMode::Polling).await;
        self.schedule_fetch().await;

        self.0.run_complete.store(true, Ordering::Release);
        self.0.run_complete_notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "runtime-tokio"))]
mod return_tests;
