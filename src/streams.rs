use std::{num::NonZeroUsize, time::Duration};

use futures::{stream, Stream};
use graphile_worker_shutdown_signal::ShutdownSignal;
use sqlx::{postgres::PgListener, PgPool};
use tracing::error;

use crate::{
    errors::Result,
    sql::{get_job::get_job, task_identifiers::TaskDetails},
    Job,
};

/// Indicates the source of a job signal that triggered job processing.
///
/// This enum represents the different mechanisms that can trigger
/// the worker to check for and process jobs.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StreamSource {
    /// Job processing was triggered by a regular polling interval
    Polling,
    /// Job processing was triggered by a PostgreSQL notification
    PgListener,
    /// Job processing came from a one-time run request
    RunOnce,
}

/// Internal data structure for managing the job signal stream.
///
/// This struct holds the state needed to produce job signals from both
/// interval-based polling and PostgreSQL notifications.
struct JobSignalStreamData {
    /// Timer for regular polling intervals
    interval: tokio::time::Interval,
    /// Listener for PostgreSQL notifications
    pg_listener: PgListener,
    /// Signal that completes when the worker should shut down
    shutdown_signal: ShutdownSignal,
    /// Number of jobs to process concurrently
    concurrency: usize,
    /// When a job signal is received, yields multiple items to allow for concurrent processing
    yield_n: Option<(NonZeroUsize, StreamSource)>,
}

impl JobSignalStreamData {
    /// Creates a new JobSignalStreamData with the given components.
    fn new(
        interval: tokio::time::Interval,
        pg_listener: PgListener,
        shutdown_signal: ShutdownSignal,
        concurrency: usize,
    ) -> Self {
        JobSignalStreamData {
            interval,
            pg_listener,
            shutdown_signal,
            concurrency,
            yield_n: None,
        }
    }
}

/// Creates a stream that yields job processing signals from multiple sources.
///
/// This function returns a stream that emits signals when the worker should check for jobs.
/// The signals come from:
/// 1. Regular interval-based polling (every `poll_interval`)
/// 2. PostgreSQL notifications when new jobs are inserted (`NOTIFY 'jobs:insert'`)
///
/// When a signal is received, the stream will emit enough items to utilize the
/// configured concurrency (one item per potential concurrent job).
///
/// The stream will terminate when the shutdown signal is triggered.
///
/// # Arguments
///
/// * `pg_pool` - PostgreSQL connection pool
/// * `poll_interval` - How often to poll for jobs when no notifications are received
/// * `shutdown_signal` - Signal that completes when the worker should shut down
/// * `concurrency` - Number of jobs to process concurrently
///
/// # Returns
///
/// A stream that emits `StreamSource` items when jobs should be checked for
pub async fn job_signal_stream(
    pg_pool: PgPool,
    poll_interval: Duration,
    shutdown_signal: ShutdownSignal,
    concurrency: usize,
) -> Result<impl Stream<Item = StreamSource>> {
    let interval = tokio::time::interval(poll_interval);

    let mut pg_listener = PgListener::connect_with(&pg_pool).await?;
    pg_listener.listen("jobs:insert").await?;
    let stream_data = JobSignalStreamData::new(interval, pg_listener, shutdown_signal, concurrency);
    let stream = stream::unfold(stream_data, |mut f| async {
        if let Some((n, source)) = f.yield_n.take() {
            if n.get() > 1 {
                let remaining_yields = n.get() - 1;
                f.yield_n = Some((NonZeroUsize::new(remaining_yields).unwrap(), source));
            } else {
                f.yield_n = None;
            }
            return Some((source, f));
        }

        tokio::select! {
            _ = (f.interval).tick() => {
                f.yield_n = Some((NonZeroUsize::new(f.concurrency).unwrap(), StreamSource::Polling));
                Some((StreamSource::Polling, f))
            },
            _ = (f.pg_listener).recv() => {
                f.yield_n = Some((NonZeroUsize::new(f.concurrency).unwrap(), StreamSource::PgListener));
                Some((StreamSource::PgListener, f))
            },
            _ = &mut f.shutdown_signal => None,
        }
    });

    Ok(stream)
}

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
/// * `task_details` - Mapping of task IDs to their string identifiers
/// * `escaped_schema` - Database schema name (properly escaped for SQL)
/// * `worker_id` - Unique identifier for this worker
/// * `forbidden_flags` - List of job flags that this worker will not process
///
/// # Returns
///
/// A stream that emits `Job` items that are ready to be processed
pub fn job_stream(
    pg_pool: PgPool,
    shutdown_signal: ShutdownSignal,
    task_details: TaskDetails,
    escaped_schema: String,
    worker_id: String,
    forbidden_flags: Vec<String>,
) -> impl Stream<Item = Job> {
    futures::stream::unfold((), move |()| {
        let pg_pool = pg_pool.clone();
        let task_details = task_details.clone();
        let escaped_schema = escaped_schema.clone();
        let worker_id = worker_id.clone();
        let forbidden_flags = forbidden_flags.clone();

        let job_fut = async move {
            let job = get_job(
                &pg_pool,
                &task_details,
                &escaped_schema,
                &worker_id,
                &forbidden_flags,
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
                    error!("Error occured while trying to get job : {:?}", job);
                    None
                }
            }
        };
        let shutdown_fut = shutdown_signal.clone();

        async move {
            tokio::select! {
                res = job_fut => res,
                _ = shutdown_fut => None
            }
        }
    })
}
