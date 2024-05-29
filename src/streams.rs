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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StreamSource {
    Polling,
    PgListener,
    RunOnce,
}

struct JobSignalStreamData {
    interval: tokio::time::Interval,
    pg_listener: PgListener,
    shutdown_signal: ShutdownSignal,
    concurrency: usize,
    yield_n: Option<(NonZeroUsize, StreamSource)>,
}

impl JobSignalStreamData {
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

/// Returns a stream that yield on postgres `NOTIFY 'jobs:insert'` and on interval specified by the
/// poll_interval argument
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
            let source = source.clone();
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

/// Returns a stream that yield every job that is available to be processed
/// It stops when the shutdown_signal is triggered or when there is no more job to process
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
