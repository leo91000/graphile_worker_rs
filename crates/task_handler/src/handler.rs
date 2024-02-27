use graphile_worker_ctx::{FromWorkerContext, WorkerContext};
use std::future::Future;

pub trait TaskHandler<T>: Send {
    fn run(
        &self,
        worker_context: WorkerContext,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static;
}

impl<F, Fut, Error> TaskHandler<()> for F
where
    Error: std::fmt::Debug + Send + 'static,
    Fut: Future<Output = Result<(), Error>> + Send + 'static,
    F: Fn() -> Fut + Send,
{
    fn run(
        &self,
        _worker_context: WorkerContext,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        let fut = self();
        async move { fut.await.map_err(|e| format!("{:?}", e)) }
    }
}

impl<F, C1, Fut, Error> TaskHandler<(C1,)> for F
where
    Error: std::fmt::Debug + Send + 'static,
    Fut: Future<Output = Result<(), Error>> + Send + 'static,
    F: Fn(C1) -> Fut + Send + Sync + 'static,
    C1: FromWorkerContext + Send + Sync + 'static,
{
    fn run(
        &self,
        worker_context: WorkerContext,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        let fut = self(C1::from_worker_context(worker_context.clone()));
        async move { fut.await.map_err(|e| format!("{:?}", e)) }
    }
}
