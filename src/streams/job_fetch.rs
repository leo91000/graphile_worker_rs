use std::sync::Arc;

use chrono::Utc;
use futures::{FutureExt, Stream, StreamExt};
use graphile_worker_database::{Database, Schema};
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::error;

use crate::Job;
use graphile_worker_queries::flag_filter::JobFlagFilter;
use graphile_worker_queries::{get_job::get_job_with_filter, task_identifiers::SharedTaskDetails};

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
/// * `database` - Database connection handle
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
    job_stream_with_filter(
        database,
        shutdown_signal,
        task_details,
        schema,
        worker_id,
        JobFlagFilter::owned(forbidden_flags, Vec::new()),
        use_local_time,
    )
}

/// Streams registered jobs satisfying a worker's flag filters.
///
/// A nonempty accepted set requires any matching flag; forbidden flags veto.
/// The filter is applied by PostgreSQL before jobs are claimed.
/// An enabled allowlist requires every job flag key to be listed and rejects
/// non-object JSON flags; untagged jobs must still satisfy accepted routing.
pub fn job_stream_with_filter<'a>(
    database: Database,
    shutdown_signal: ShutdownSignal,
    task_details: SharedTaskDetails,
    schema: Schema,
    worker_id: String,
    filter: JobFlagFilter<'a>,
    use_local_time: bool,
) -> impl Stream<Item = Job> + 'a {
    try_job_stream_with_filter(
        database,
        shutdown_signal,
        task_details,
        schema,
        worker_id,
        filter,
        use_local_time,
    )
    .filter_map(|result| async move { result.ok() })
}

/// Yields a fetch error once before terminating, allowing callers to drain active jobs.
pub(crate) fn try_job_stream_with_filter<'a>(
    database: Database,
    shutdown_signal: ShutdownSignal,
    task_details: SharedTaskDetails,
    schema: Schema,
    worker_id: String,
    filter: JobFlagFilter<'a>,
    use_local_time: bool,
) -> impl Stream<Item = crate::errors::Result<Job>> + 'a {
    let filter = Arc::new(filter);
    futures::stream::unfold(false, move |finished| {
        let database = database.clone();
        let task_details = task_details.clone();
        let schema = schema.clone();
        let worker_id = worker_id.clone();
        let filter = filter.clone();

        let job_fut = async move {
            if finished {
                return None;
            }
            let now = use_local_time.then(Utc::now);
            let task_details_guard = task_details.read().await;
            let job = get_job_with_filter(
                &database,
                &task_details_guard,
                &schema,
                &worker_id,
                filter.as_borrowed(),
                now,
            )
            .await
            .map_err(|e| {
                error!("Could not get job : {:?}", e);
                e
            });

            match job {
                Ok(Some(job)) => Some((Ok(job), false)),
                Ok(None) => None,
                Err(error) => Some((Err(error), true)),
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
