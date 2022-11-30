use std::time::Duration;

use futures::{stream, Stream, StreamExt};
use sqlx::{postgres::PgListener, PgPool};

use crate::errors::Result;

#[derive(Debug)]
pub enum StreamSource {
    Polling,
    PgListener,
}

fn interval_stream(time: Duration) -> impl Stream<Item = StreamSource> {
    stream::unfold(time, |time| async move {
        tokio::time::sleep(time).await;
        Some((StreamSource::Polling, time))
    })
}

async fn job_notification_stream(pg_pool: PgPool) -> Result<impl Stream<Item = StreamSource>> {
    let mut listener = PgListener::connect_with(&pg_pool).await?;
    listener.listen("jobs:insert").await?;

    let stream = listener
        .into_stream()
        .filter_map(|v| async move { v.map(|_| StreamSource::PgListener).ok() });

    Ok(stream)
}

/// Returns a stream that yield on postgres `NOTIFY 'jobs:insert'` and on interval specified by the
/// poll_interval argument
pub async fn job_signal_stream(
    pg_pool: PgPool,
    poll_interval: Duration,
) -> Result<impl Stream<Item = StreamSource>> {
    let interval_stream = interval_stream(poll_interval);
    let job_notification_stream = job_notification_stream(pg_pool).await?;

    Ok(stream::select(interval_stream, job_notification_stream))
}
