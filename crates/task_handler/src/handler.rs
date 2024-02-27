use graphile_worker_ctx::WorkerContext;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use std::future::Future;

pub trait TaskHandler: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {
    const IDENTIFIER: &'static str;

    fn run(
        self,
        ctx: WorkerContext,
    ) -> impl Future<Output = Result<(), impl Debug>> + Send + 'static;

    fn run_from_ctx(
        worker_context: WorkerContext,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        let job = serde_json::from_value::<Self>(worker_context.payload().clone());
        async move {
            let Ok(job) = job else {
                let e = job.err().unwrap();
                return Err(format!("{e:?}"));
            };
            job.run(worker_context).await.map_err(|e| format!("{e:?}"))
        }
    }
}
