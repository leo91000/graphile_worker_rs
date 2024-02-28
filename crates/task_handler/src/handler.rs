use graphile_worker_ctx::WorkerContext;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use std::future::Future;

pub trait TaskHandler<AS: Sync + Send = ()>:
    Serialize + for<'de> Deserialize<'de> + Send + Sync
{
    const IDENTIFIER: &'static str;

    fn run(self, ctx: WorkerContext<AS>) -> impl Future<Output = Result<(), impl Debug>> + Send;

    fn run_from_ctx(
        worker_context: WorkerContext<AS>,
    ) -> impl Future<Output = Result<(), String>> + Send {
        let job = serde_json::from_value::<Self>(worker_context.job().payload().clone());
        async move {
            let Ok(job) = job else {
                let e = job.err().unwrap();
                return Err(format!("{e:?}"));
            };
            job.run(worker_context).await.map_err(|e| format!("{e:?}"))
        }
    }
}
