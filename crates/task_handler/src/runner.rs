use serde::{Deserialize, Serialize};
use std::{fmt::Debug, future::Future, time::Instant};
use tokio_util::sync::CancellationToken;

use crate::task_result::{RunTaskError, SpawnTaskResult};

pub trait TaskHandler<Payload, Context>
where
    Payload: for<'de> Deserialize<'de> + Serialize + Send,
    Context: Send,
{
    fn run(
        &self,
        payload: Payload,
        ctx: Context,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static;

    fn identifier(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn spawn_task(
        &self,
        payload: Payload,
        ctx: Context,
        cancel_token: CancellationToken,
    ) -> impl Future<Output = SpawnTaskResult<String>> {
        async move {
            let start = Instant::now();

            let task_fut = async {
                tokio::spawn(self.run(payload, ctx))
                    .await
                    .map_err(|_| RunTaskError::TaskPanic)
                    .and_then(|r| r.map_err(RunTaskError::TaskError))
            };
            let cancel_fut = async {
                cancel_token.cancelled().await;
            };

            let result = tokio::select! {
                _ = cancel_fut => Err(RunTaskError::TaskAborted),
                r = task_fut => r,
            };
            let duration = start.elapsed();
            SpawnTaskResult { duration, result }
        }
    }
}

impl<Payload, Context, Error, F> TaskHandler<Payload, Context> for F
where
    Payload: for<'de> Deserialize<'de> + Serialize + Send,
    Context: Send,
    Error: Debug + Send,
    F: Fn(Context, Payload),
    F::Output: Future<Output = Result<(), Error>> + Send + 'static,
{
    fn run(
        &self,
        payload: Payload,
        ctx: Context,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        let res = (self)(ctx, payload);
        async move { res.await.map_err(|e| format!("{:?}", e)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn task_fn(_payload: (), _ctx: ()) -> Result<(), ()> {
        Ok(())
    }

    fn assert_task_handler<Payload, Context, T>(_: T)
    where
        Payload: for<'de> Deserialize<'de> + Serialize + Send,
        Context: Send,
        T: TaskHandler<Payload, Context>,
    {
    }

    #[tokio::test]
    async fn test_task_handler() {
        assert_task_handler(task_fn);
    }
}
