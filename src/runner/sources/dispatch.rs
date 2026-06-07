use futures::{stream::FuturesUnordered, FutureExt, Stream, StreamExt};
use graphile_worker_runtime as runtime;

use super::super::errors::{ProcessJobError, WorkerRuntimeError};
use crate::streams::StreamSource;

pub(in crate::runner) async fn dispatch_job_signals<S>(
    job_signal: S,
    source_tx: runtime::Sender<StreamSource>,
    mut worker_handles: FuturesUnordered<runtime::JoinHandle<Result<(), ProcessJobError>>>,
    fanout: usize,
) -> Result<(), WorkerRuntimeError>
where
    S: Stream<Item = StreamSource>,
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

                let mut closed = false;
                for _ in 0..fanout {
                    if source_tx.try_send(source).is_err() && source_tx.send(source).await.is_err() {
                        closed = true;
                        break;
                    }
                }
                if closed {
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
