use futures::{stream::FuturesUnordered, FutureExt, Stream, StreamExt};
use graphile_worker_runtime as runtime;
use tracing::warn;

use super::super::errors::{ProcessJobError, WorkerRuntimeError};
use crate::streams::job_signal::JobSignalSource;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FanoutResult {
    Open,
    Closed,
}

/// Fans job signals out to the worker's job slots until the signal stream ends
/// or a slot fails.
///
/// A failed slot shuts the worker down: the other slots' jobs get the shutdown
/// grace period, and this returns only once every slot has stopped, so no job
/// keeps running after `Worker::run` has returned.
pub(in crate::runner) async fn dispatch_job_signals<S>(
    job_signal: S,
    source_tx: runtime::Sender<JobSignalSource>,
    mut worker_handles: FuturesUnordered<runtime::JoinHandle<Result<(), ProcessJobError>>>,
    fanout: usize,
    shutdown_notifier: &runtime::Notify,
) -> Result<(), WorkerRuntimeError>
where
    S: Stream<Item = JobSignalSource>,
{
    let job_signal = job_signal.fuse();
    futures::pin_mut!(job_signal);

    let mut first_error = None;

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
                    Some(result) => {
                        first_error = slot_error(result);
                        shutdown_notifier.notify_one();
                        break;
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

    source_tx.close();

    while let Some(result) = worker_handles.next().await {
        let Some(error) = slot_error(result) else {
            continue;
        };

        if first_error.is_some() {
            warn!(error = %error, "Job slot failed while the worker was stopping");
            continue;
        }

        shutdown_notifier.notify_one();
        first_error = Some(error);
    }

    first_error.map_or(Ok(()), Err)
}

fn slot_error(
    result: Result<Result<(), ProcessJobError>, runtime::JoinError>,
) -> Option<WorkerRuntimeError> {
    match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error.into()),
        Err(error) => Some(error.into()),
    }
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

    #[tokio::test]
    async fn slot_failure_requests_shutdown_and_waits_for_the_other_slots() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        use crate::errors::GraphileWorkerError;

        let shutdown_notifier = Arc::new(runtime::Notify::new());
        let other_slot_stopped = Arc::new(AtomicBool::new(false));
        let (source_tx, _source_rx) = runtime::channel(1);
        let worker_handles = FuturesUnordered::new();

        worker_handles.push(runtime::spawn(async {
            Err(ProcessJobError::GetJobError(
                GraphileWorkerError::JobScheduleSkipped,
            ))
        }));
        worker_handles.push(runtime::spawn({
            let shutdown_notifier = shutdown_notifier.clone();
            let other_slot_stopped = other_slot_stopped.clone();
            async move {
                shutdown_notifier.notified().await;
                runtime::sleep(Duration::from_millis(50)).await;
                other_slot_stopped.store(true, Ordering::SeqCst);
                Ok(())
            }
        }));

        let result = dispatch_job_signals(
            futures::stream::pending(),
            source_tx,
            worker_handles,
            1,
            &shutdown_notifier,
        )
        .await;

        assert!(matches!(
            result,
            Err(WorkerRuntimeError::ProcessJob(
                ProcessJobError::GetJobError(GraphileWorkerError::JobScheduleSkipped)
            ))
        ));
        assert!(other_slot_stopped.load(Ordering::SeqCst));
    }
}
