use futures::try_join;
use graphile_worker_lifecycle_hooks::{ShutdownReason, WorkerShutdownContext, WorkerStartContext};
use tracing::warn;

use super::super::{Worker, WorkerRuntimeError};
use crate::recovery_tasks::{deregister_worker, register_worker, spawn_recovery_tasks};

impl Worker {
    /// Runs the worker until the shutdown signal is triggered.
    pub async fn run(&self) -> Result<(), WorkerRuntimeError> {
        register_worker(self, None).await?;
        self.emit_start().await;

        let recovery_tasks = spawn_recovery_tasks(self);
        let result = self.run_job_sources().await;

        self.await_batchers().await;
        recovery_tasks.stop().await;
        self.emit_shutdown(result_to_shutdown_reason(&result)).await;

        finish_run(result, deregister_worker(self).await)
    }

    async fn run_job_sources(&self) -> Result<(), WorkerRuntimeError> {
        let local_queue = self.create_local_queues();
        let job_runner = self.job_runner_internal(local_queue);
        let crontab_scheduler = self.crontab_scheduler();

        try_join!(crontab_scheduler, job_runner).map(|_| ())
    }

    async fn emit_start(&self) {
        self.hooks
            .emit(WorkerStartContext {
                database: self.database.clone(),
                worker_id: self.worker_id.clone(),
                extensions: self.extensions.clone(),
            })
            .await;
    }

    async fn emit_shutdown(&self, reason: ShutdownReason) {
        self.hooks
            .emit(WorkerShutdownContext {
                database: self.database.clone(),
                worker_id: self.worker_id.clone(),
                reason,
            })
            .await;
    }

    async fn await_batchers(&self) {
        if let Some(batcher) = &self.completion_batcher {
            batcher.await_shutdown().await;
        }
        if let Some(batcher) = &self.failure_batcher {
            batcher.await_shutdown().await;
        }
    }
}

fn result_to_shutdown_reason(result: &Result<(), WorkerRuntimeError>) -> ShutdownReason {
    match result {
        Ok(()) => ShutdownReason::Graceful,
        Err(_) => ShutdownReason::Error,
    }
}

fn finish_run(
    result: Result<(), WorkerRuntimeError>,
    deregister_result: Result<(), crate::errors::GraphileWorkerError>,
) -> Result<(), WorkerRuntimeError> {
    match (result, deregister_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(error)) => Err(error.into()),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => {
            warn!(
                error = %cleanup_error,
                "Failed to deregister worker after runtime error"
            );
            Err(error)
        }
    }
}
