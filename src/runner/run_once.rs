use std::sync::Arc;

use chrono::Utc;
use futures::StreamExt;
use graphile_worker_runtime::Mutex;
use tracing::{error, info};

use super::errors::ProcessJobError;
use super::job_execution::run_and_release_job;
use super::{Worker, WorkerRuntimeError};
use crate::streams::job_signal::JobSignalSource;
use crate::streams::try_job_stream_with_filter;
use graphile_worker_queries::get_job::get_job_with_filter;

impl Worker {
    /// Runs the worker once and processes all available jobs, then returns.
    ///
    /// Returns the first fetch or outcome-persistence error after draining active work.
    /// Requests accepted by an open batcher remain asynchronous.
    pub async fn run_once(&self) -> Result<(), WorkerRuntimeError> {
        let job_stream = try_job_stream_with_filter(
            self.database.clone(),
            self.shutdown_signal.clone(),
            self.task_details.clone(),
            self.schema.clone(),
            self.worker_id.clone(),
            self.job_flag_filter.as_borrowed(),
            self.use_local_time,
        );

        let runner = self.runner();
        let run_error = Mutex::new(None);

        job_stream
            .for_each_concurrent(self.concurrency, {
                let runner = runner.clone();
                let run_error = &run_error;
                move |job| {
                    let runner = runner.clone();
                    async move {
                        let mut job = match job {
                            Ok(job) => job,
                            Err(error) => {
                                run_error.lock().await.get_or_insert(error.into());
                                return;
                            }
                        };
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
                                    let e = ProcessJobError::from(e);
                                    error!("Error while processing job : {:?}", e);
                                    run_error.lock().await.get_or_insert(e);
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
                            .await;
                            drop(task_details_guard);
                            match new_job {
                                Ok(Some(new_job)) => job = new_job,
                                Ok(None) => break,
                                Err(error) => {
                                    run_error.lock().await.get_or_insert(error.into());
                                    break;
                                }
                            }
                        }
                    }
                }
            })
            .await;

        if let Some(error) = run_error.into_inner() {
            return Err(error.into());
        }
        Ok(())
    }
}
