use std::sync::Arc;

use chrono::Utc;
use futures::StreamExt;
use tracing::{error, info};

use super::job_execution::run_and_release_job;
use super::{Worker, WorkerRuntimeError};
use crate::sql::get_job::get_job;
use crate::streams::{job_stream, StreamSource};

impl Worker {
    /// Runs the worker once and processes all available jobs, then returns.
    pub async fn run_once(&self) -> Result<(), WorkerRuntimeError> {
        let job_stream = job_stream(
            self.database.clone(),
            self.shutdown_signal.clone(),
            self.task_details.clone(),
            self.schema.clone(),
            self.worker_id.clone(),
            self.forbidden_flags.clone(),
            self.use_local_time,
        );

        let runner = self.runner();

        job_stream
            .for_each_concurrent(self.concurrency, {
                let runner = runner.clone();
                move |mut job| {
                    let runner = runner.clone();
                    async move {
                        loop {
                            let job_id = *job.id();
                            let has_queue = job.job_queue_id().is_some();
                            let result =
                                run_and_release_job(Arc::new(job), &runner, &StreamSource::RunOnce)
                                    .await;

                            match result {
                                Ok(_) => {
                                    info!(job_id, "Job processed");
                                }
                                Err(e) => {
                                    error!("Error while processing job : {:?}", e);
                                }
                            };

                            if !has_queue {
                                break;
                            }
                            info!(job_id, "Job has queue, fetching another job");
                            let now = runner.use_local_time.then(Utc::now);
                            let task_details_guard = runner.task_details.read().await;
                            let new_job = get_job(
                                &runner.database,
                                &task_details_guard,
                                &runner.schema,
                                &runner.worker_id,
                                &runner.forbidden_flags,
                                now,
                            )
                            .await
                            .unwrap_or(None);
                            drop(task_details_guard);
                            let Some(new_job) = new_job else {
                                break;
                            };
                            job = new_job;
                        }
                    }
                }
            })
            .await;

        Ok(())
    }
}
