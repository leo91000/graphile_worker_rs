use std::time::Duration;

use archimedes_shutdown_signal::ShutdownSignal;
use futures::{stream, Stream};
use sqlx::{postgres::PgListener, PgPool};

use crate::errors::Result;

#[derive(Debug)]
pub enum StreamSource {
    Polling,
    PgListener,
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
            _ = (&mut f.0).tick() => Some((StreamSource::Polling, f)),
            _ = (&mut f.1).recv() => Some((StreamSource::PgListener, f)),
            _ = &mut f.2 => None,
        }
    });

    Ok(stream)
}
