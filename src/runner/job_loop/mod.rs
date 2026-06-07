mod cron;
mod direct;
mod local_queue;

use super::{sources, Worker, WorkerRunner, WorkerRuntimeError};
use crate::local_queue::LocalQueue;

impl Worker {
    pub(super) fn create_local_queues(
        &self,
    ) -> Option<(Vec<LocalQueue>, crate::streams::JobSignalReceiver)> {
        sources::create_local_queues(self)
    }

    pub(super) fn runner(&self) -> WorkerRunner {
        WorkerRunner {
            worker_id: self.worker_id.clone(),
            jobs: self.jobs.clone(),
            database: self.database.clone(),
            schema: self.schema.clone(),
            task_details: self.task_details.clone(),
            forbidden_flags: self.forbidden_flags.clone(),
            use_local_time: self.use_local_time,
            shutdown_signal: self.shutdown_signal.clone(),
            extensions: self.extensions.clone(),
            hooks: self.hooks.clone(),
            completion_batcher: self.completion_batcher.clone(),
            failure_batcher: self.failure_batcher.clone(),
            recovery_config: self.recovery_config.clone(),
        }
    }

    pub(super) async fn job_runner_internal(
        &self,
        local_queue: Option<(Vec<LocalQueue>, crate::streams::JobSignalReceiver)>,
    ) -> Result<(), WorkerRuntimeError> {
        match local_queue {
            Some((local_queues, rx)) => local_queue::run(self, local_queues, rx).await,
            None => direct::run(self).await,
        }
    }

    pub(super) async fn crontab_scheduler(&self) -> Result<(), WorkerRuntimeError> {
        cron::run(self).await
    }
}
