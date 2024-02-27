use getset::Getters;
use graphile_worker_job::Job;
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;

#[derive(Getters, Clone, Debug)]
#[getset(get = "pub")]
pub struct WorkerContext {
    payload: Value,
    pg_pool: PgPool,
    job: Job,
    worker_id: String,
}

impl WorkerContext {
    pub fn new(payload: Value, pg_pool: PgPool, job: Job, worker_id: String) -> Self {
        WorkerContext {
            payload,
            pg_pool,
            job,
            worker_id,
        }
    }
}

pub struct WorkerId(String);
pub struct Payload<T: for<'de> Deserialize<'de>>(T);

pub trait FromWorkerContext {
    fn from_worker_context(ctx: WorkerContext) -> Self;
}

impl FromWorkerContext for PgPool {
    fn from_worker_context(ctx: WorkerContext) -> Self {
        ctx.pg_pool
    }
}

impl FromWorkerContext for Job {
    fn from_worker_context(ctx: WorkerContext) -> Self {
        ctx.job
    }
}

impl FromWorkerContext for WorkerId {
    fn from_worker_context(ctx: WorkerContext) -> Self {
        WorkerId(ctx.worker_id)
    }
}

impl<T: for<'de> Deserialize<'de>> FromWorkerContext for Payload<T> {
    fn from_worker_context(ctx: WorkerContext) -> Self {
        Payload(serde_json::from_value(ctx.payload).unwrap())
    }
}

impl FromWorkerContext for WorkerContext {
    fn from_worker_context(ctx: WorkerContext) -> Self {
        ctx
    }
}
