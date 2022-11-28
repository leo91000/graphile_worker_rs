use std::{fmt::Debug, future::ready, marker::PhantomData, pin::Pin};

use async_trait::async_trait;
use futures::{future::LocalBoxFuture, Future, FutureExt, TryFutureExt};
use getset::Getters;
use serde::Deserialize;

#[derive(Getters)]
#[getset(get = "pub")]
pub struct JobCtx {
    pool: sqlx::PgPool,
}

pub struct Payload<T: for<'de> Deserialize<'de>>(T);

impl<T: for<'de> Deserialize<'de>> Payload<T> {
    fn from_str(s: &str) -> serde_json::Result<Self> {
        Ok(Self(serde_json::from_str(s)?))
    }
}

pub trait JobHandler<O, E: Debug + From<serde_json::Error>, Fut: Future<Output = Result<O, E>>> {
    fn handler(&self, ctx: JobCtx, payload: &str) -> Fut;
}

struct JobFn<T, O, E, Fut, F>
where
    E: Debug + From<serde_json::Error>,
    Fut: Future<Output = Result<O, E>>,
    T: for<'de> Deserialize<'de>,
    F: Fn(JobCtx, Payload<T>) -> Fut,
{
    job_fn: F,
    t: PhantomData<T>,
    o: PhantomData<O>,
    e: PhantomData<E>,
    fut: PhantomData<Fut>,
}

impl<'a, T, O, E, Fut2, F> JobHandler<O, E, LocalBoxFuture<'a, Result<O, E>>> for JobFn<T, O, E, Fut2, F>
where
    E: Debug + From<serde_json::Error>,
    T: for<'de> Deserialize<'de>,
    Fut2: Future<Output = Result<O, E>>,
    F: Fn(JobCtx, Payload<T>) -> Fut2,
{
    fn handler(&self, ctx: JobCtx, payload: &str) -> LocalBoxFuture<'a, Result<O, E>> {
        ready(Payload::from_str(payload).map_err(|e| E::from(e)))
            .and_then(|de_payload| (self.job_fn)(ctx, de_payload))
            .boxed_local()
    }
}
