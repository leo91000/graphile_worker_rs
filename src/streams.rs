use std::time::Duration;

use archimedes_shutdown_signal::ShutdownSignal;
use futures::{stream, Stream};
use sqlx::{postgres::PgListener, PgPool};
use tracing::error;

use crate::{
    errors::Result,
    sql::{
        get_job::{get_job, Job},
        task_identifiers::TaskDetails,
    },
};

#[derive(Debug)]
pub enum StreamSource {
    Polling,
    PgListener,
    RunOnce,
}

/// Returns a stream that yield on postgres `NOTIFY 'jobs:insert'` and on interval specified by the
/// poll_interval argument
pub async fn job_signal_stream(
    pg_pool: PgPool,
    poll_interval: Duration,
    shutdown_signal: ShutdownSignal,
) -> Result<impl Stream<Item = StreamSource>> {
    let interval = tokio::time::interval(poll_interval);

    let mut pg_listener = PgListener::connect_with(&pg_pool).await?;
    pg_listener.listen("jobs:insert").await?;

    let stream = stream::unfold((interval, pg_listener, shutdown_signal), |mut f| async {
        tokio::select! {
            _ = (f.0).tick() => Some((StreamSource::Polling, f)),
            _ = (f.1).recv() => Some((StreamSource::PgListener, f)),
            _ = &mut f.2 => None,
        }
    });

    Ok(stream)
}

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
