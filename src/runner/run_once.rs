use std::sync::Arc;

use chrono::Utc;
use futures::StreamExt;
use graphile_worker_runtime::Mutex;
use tracing::{error, info};

use super::errors::ProcessJobError;
use super::job_execution::run_and_release_job;
use super::{Worker, WorkerRuntimeError};
use crate::streams::job_signal::JobSignalSource;
use crate::streams::job_stream_with_filter;
use graphile_worker_queries::get_job::get_job_with_filter;

impl Worker {
    /// Runs the worker once and processes all available jobs, then returns.
    ///
    /// Returns the first outcome-persistence error after draining active work.
    /// Requests accepted by an open batcher remain asynchronous.
    pub async fn run_once(&self) -> Result<(), WorkerRuntimeError> {
        let job_stream = job_stream_with_filter(
            self.database.clone(),
            self.shutdown_signal.clone(),
            self.task_details.clone(),
            self.schema.clone(),
            self.worker_id.clone(),
            self.job_flag_filter.as_borrowed(),
            self.use_local_time,
        );

        let runner = self.runner();
        let release_error = Mutex::new(None);

        job_stream
            .for_each_concurrent(self.concurrency, {
                let runner = runner.clone();
                let release_error = &release_error;
                move |mut job| {
                    let runner = runner.clone();
                    async move {
                        loop {
                            let job_id = *job.id();
                            let has_queue = job.job_queue_id().is_some();
                            let result = run_and_release_job(
                                Arc::new(job),
                                &runner,
                                &JobSignalSource::RunOnce,
                            )
                            .await;

                            match result {
                                Ok(_) => {
                                    info!(job_id, "Job processed");
                                }
                                Err(e) => {
                                    error!("Error while processing job : {:?}", e);
                                    if let error @ ProcessJobError::ReleaseJobError(_) = e {
                                        release_error.lock().await.get_or_insert(error);
                                    }
                                }
                            };

                            if !has_queue {
                                break;
                            }
                            info!(job_id, "Job has queue, fetching another job");
                            let now = runner.use_local_time.then(Utc::now);
                            let task_details_guard = runner.task_details.read().await;
                            let new_job = get_job_with_filter(
                                &runner.database,
                                &task_details_guard,
                                &runner.schema,
                                &runner.worker_id,
                                runner.job_flag_filter.as_borrowed(),
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

        if let Some(error) = release_error.into_inner() {
            return Err(error.into());
        }
        Ok(())
    }
}
