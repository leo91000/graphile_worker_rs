use futures::{stream::FuturesUnordered, FutureExt, Stream, StreamExt};
use graphile_worker_runtime as runtime;

use super::super::errors::{ProcessJobError, WorkerRuntimeError};
use crate::streams::job_signal::JobSignalSource;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FanoutResult {
    Open,
    Closed,
}

pub(in crate::runner) async fn dispatch_job_signals<S>(
    job_signal: S,
    source_tx: runtime::Sender<JobSignalSource>,
    mut worker_handles: FuturesUnordered<runtime::JoinHandle<Result<(), ProcessJobError>>>,
    fanout: usize,
) -> Result<(), WorkerRuntimeError>
where
    S: Stream<Item = JobSignalSource>,
{
    let job_signal = job_signal.fuse();
    futures::pin_mut!(job_signal);

    loop {
        let next_source = job_signal.next().fuse();
        let worker_done = worker_handles.next().fuse();
        futures::pin_mut!(next_source, worker_done);

        futures::select_biased! {
            worker_result = worker_done => {
                match worker_result {
                    Some(Ok(Ok(()))) => {
                        if worker_handles.is_empty() {
                            break;
                        }
                    }
                    Some(Ok(Err(e))) => {
                        source_tx.close();
                        return Err(e.into());
                    }
                    Some(Err(e)) => {
                        source_tx.close();
                        return Err(e.into());
                    }
                    None => break,
                }
            }
            source = next_source => {
                let Some(source) = source else {
                    break;
                };

                if fanout_job_signal(&source_tx, source, fanout) == FanoutResult::Closed {
                    break;
                }
            }
        }
    }

    drop(source_tx);

    while let Some(result) = worker_handles.next().await {
        result??;
    }

    Ok(())
}

fn fanout_job_signal(
    source_tx: &runtime::Sender<JobSignalSource>,
    source: JobSignalSource,
    fanout: usize,
) -> FanoutResult {
    for _ in 0..fanout {
        match source_tx.try_send(source) {
            Ok(()) => {}
            Err(error) if error.is_closed() => return FanoutResult::Closed,
            Err(_) => return FanoutResult::Open,
        }
    }

    FanoutResult::Open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fanout_queues_requested_signals_when_channel_has_capacity() {
        let (tx, rx) = runtime::channel(4);

        let result = fanout_job_signal(&tx, JobSignalSource::Notification, 3);

        assert_eq!(result, FanoutResult::Open);
        assert!(matches!(rx.try_recv(), Ok(JobSignalSource::Notification)));
        assert!(matches!(rx.try_recv(), Ok(JobSignalSource::Notification)));
        assert!(matches!(rx.try_recv(), Ok(JobSignalSource::Notification)));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn fanout_coalesces_when_worker_channel_is_full() {
        let (tx, rx) = runtime::channel(1);
        tx.try_send(JobSignalSource::Polling)
            .expect("initial signal should fit");

        let result = fanout_job_signal(&tx, JobSignalSource::Notification, 3);

        assert_eq!(result, FanoutResult::Open);
        assert!(matches!(rx.try_recv(), Ok(JobSignalSource::Polling)));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn fanout_reports_closed_worker_channel() {
        let (tx, rx) = runtime::channel(1);
        drop(rx);

        let result = fanout_job_signal(&tx, JobSignalSource::Notification, 1);

        assert_eq!(result, FanoutResult::Closed);
    }
}
