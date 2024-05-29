use graphile_worker_ctx::WorkerContext;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use std::future::Future;

pub trait IntoTaskHandlerResult {
    fn into_task_handler_result(self) -> Result<(), impl Debug>;
}

impl IntoTaskHandlerResult for () {
    fn into_task_handler_result(self) -> Result<(), impl Debug> {
        Ok::<_, ()>(())
    }
}

impl<D: Debug> IntoTaskHandlerResult for Result<(), D> {
    fn into_task_handler_result(self) -> Result<(), impl Debug> {
        self
    }
}

pub trait TaskHandler: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {
    const IDENTIFIER: &'static str;

    fn run(
        self,
        ctx: WorkerContext,
    ) -> impl Future<Output = impl IntoTaskHandlerResult> + Send + 'static;
}

pub async fn run_task_from_worker_ctx<T: TaskHandler>(
    worker_context: WorkerContext,
) -> Result<(), String> {
    let job = serde_json::from_value::<T>(worker_context.payload().clone());
    let Ok(job) = job else {
        let e = job.err().unwrap();
        return Err(format!("{e:?}"));
    };
    job.run(worker_context)
        .await
        .into_task_handler_result()
        .map_err(|e| format!("{e:?}"))
}
