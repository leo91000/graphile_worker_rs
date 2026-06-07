use chrono::Utc;
use futures::{FutureExt, Stream};
use graphile_worker_database::{Database, Schema};
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::error;

use crate::sql::{get_job::get_job, task_identifiers::SharedTaskDetails};
use crate::Job;

/// Creates a stream that yields jobs ready for processing.
///
/// This function returns a stream that fetches and yields jobs from the database
/// that are ready to be processed. It will continue to emit jobs until either:
/// 1. There are no more jobs available to process
/// 2. The shutdown signal is triggered
///
/// The stream is typically used with `for_each_concurrent` to process multiple
/// jobs in parallel up to the configured concurrency limit.
///
/// # Arguments
///
/// * `pg_pool` - PostgreSQL connection pool
/// * `shutdown_signal` - Signal that completes when the worker should shut down
/// * `task_details` - Shared mapping of task IDs to their string identifiers
/// * `schema` - Database schema where Graphile Worker tables are located.
/// * `worker_id` - Unique identifier for this worker
/// * `forbidden_flags` - List of job flags that this worker will not process
/// * `use_local_time` - Whether to use local application time (true) or database time (false)
///
/// # Returns
///
/// A stream that emits `Job` items that are ready to be processed
pub fn job_stream(
    database: Database,
    shutdown_signal: ShutdownSignal,
    task_details: SharedTaskDetails,
    schema: Schema,
    worker_id: String,
    forbidden_flags: Vec<String>,
    use_local_time: bool,
) -> impl Stream<Item = Job> {
    futures::stream::unfold((), move |()| {
        let database = database.clone();
        let task_details = task_details.clone();
        let schema = schema.clone();
        let worker_id = worker_id.clone();
        let forbidden_flags = forbidden_flags.clone();

        let job_fut = async move {
            let now = use_local_time.then(Utc::now);
            let task_details_guard = task_details.read().await;
            let job = get_job(
                &database,
                &task_details_guard,
                &schema,
                &worker_id,
                &forbidden_flags,
                now,
            )
            .await
            .map_err(|e| {
                error!("Could not get job : {:?}", e);
                e
            });

            match job {
                Ok(Some(job)) => Some((job, ())),
                Ok(None) => None,
                Err(_) => {
                    error!("Error occurred while trying to get job : {:?}", job);
                    None
                }
            }
        };
        let shutdown_fut = shutdown_signal.clone();

        async move {
            let job_fut = job_fut.fuse();
            let shutdown_fut = shutdown_fut.fuse();
            futures::pin_mut!(job_fut, shutdown_fut);

            futures::select_biased! {
                res = job_fut => res,
                _ = shutdown_fut => None
            }
        }
    })
}
