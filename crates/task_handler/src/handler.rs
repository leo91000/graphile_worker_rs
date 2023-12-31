use serde::{Deserialize, Serialize};
use std::{fmt::Debug, future::Future};

pub trait TaskHandler<Payload, Context>: Send
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
}

impl<Payload, Context, Error, F, Fut> TaskHandler<Payload, Context> for F
where
    Payload: for<'de> Deserialize<'de> + Serialize + Send,
    Context: Send,
    Error: Debug + Send,
    Fut: Future<Output = Result<(), Error>> + Send + 'static,
    F: Fn(Payload, Context) -> Fut + Send,
{
    fn run(
        &self,
        payload: Payload,
        ctx: Context,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        let res = (self)(payload, ctx);
        async move { res.await.map_err(|e| format!("{:?}", e)) }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn assert_task_handler<Payload, Context, T>(_: T)
    where
        Payload: for<'de> Deserialize<'de> + Serialize + Send,
        Context: Send,
        T: TaskHandler<Payload, Context>,
    {
    }

    async fn task_fn(_payload: (), _ctx: ()) -> Result<(), ()> {
        Ok(())
    }

    async fn task_fn_with_error(_payload: (), _ctx: ()) -> Result<(), i32> {
        Err(1)
    }

    #[test]
    fn test_task_handler() {
        assert_task_handler(task_fn);
        assert_task_handler(task_fn_with_error);
    }

    #[test]
    fn test_task_identifier() {
        let crate_name = env!("CARGO_PKG_NAME");
        assert_eq!(
            task_fn.identifier(),
            format!("{crate_name}::handler::test::task_fn")
        );
        assert_eq!(
            task_fn_with_error.identifier(),
            format!("{crate_name}::handler::test::task_fn_with_error")
        );
    }
}
