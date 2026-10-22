use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use chrono::Utc;
use futures::StreamExt;
use graphile_worker_runtime::Mutex;
use tracing::{error, info};

use super::errors::{ProcessJobError, ReleaseJobError};
use super::job_execution::run_and_release_job;
use super::{Worker, WorkerRuntimeError};
use crate::streams::job_signal::JobSignalSource;
use crate::streams::try_job_stream_with_filter;
use graphile_worker_queries::{get_job::get_job_with_filter, return_jobs::batch::return_jobs};

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

        // Stop both sources after a fatal fetch, without cancelling a claim that
        // might already have committed. Such claims are returned before exit.
        let fetch_failed = AtomicBool::new(false);
        let fetch_failed = &fetch_failed;
        let run_error = Mutex::new(None);
        let job_stream = futures::stream::unfold(Box::pin(job_stream), {
            let run_error = &run_error;
            move |mut stream| async move {
                if fetch_failed.load(Ordering::Acquire) {
                    return None;
                }
                let job = match stream.next().await? {
                    Ok(job) => job,
                    Err(error) => {
                        // Record before polling another ready continuation.
                        fetch_failed.store(true, Ordering::Release);
                        run_error.lock().await.get_or_insert(error.into());
                        return None;
                    }
                };
                Some((job, stream))
            }
        });
        let runner = self.runner();

        job_stream
            .for_each_concurrent(self.concurrency, {
                let runner = runner.clone();
                let run_error = &run_error;
                move |job| {
                    let runner = runner.clone();
                    async move {
                        let mut job = job;
                        loop {
                            if fetch_failed.load(Ordering::Acquire) {
                                if let Err(source) = return_jobs(
                                    &runner.database,
                                    std::slice::from_ref(&job),
                                    &runner.schema,
                                    &runner.worker_id,
                                ).await {
                                    error!(job_id = *job.id(), %source, "Could not return unstarted claim after fetch failure");
                                    run_error.lock().await.get_or_insert(ReleaseJobError {
                                        job_id: *job.id(), source,
                                    }.into());
                                }
                                break;
                            }
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

                            if !has_queue || fetch_failed.load(Ordering::Acquire) {
                                break;
                            }
                            info!(job_id, "Job has queue, fetching another job");
                            let now = runner.use_local_time.then(Utc::now);
                            let task_details_guard = runner.task_details.read().await;
                            if fetch_failed.load(Ordering::Acquire) {
                                break;
                            }
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
                                    fetch_failed.store(true, Ordering::Release);
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
