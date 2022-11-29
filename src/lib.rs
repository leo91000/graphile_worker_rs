use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures::FutureExt;
use serde::Deserialize;

pub mod context;
mod db;
pub mod errors;
pub mod migrate;
mod migrations;
mod sql;
mod utils;

#[derive(Clone)]
pub struct WorkerContext {
    pool: sqlx::PgPool,
}

type WorkerFn = Box<
    dyn Fn(WorkerContext, Arc<String>) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>,
>;

pub struct Worker {
    concurrency: usize,
    poll_interval: Duration,
    jobs: HashMap<String, WorkerFn>,
}

impl Worker {
    pub fn builder() -> WorkerBuilder {
        WorkerBuilder::default()
    }

    pub async fn start(&self) {
        let interval = tokio::time::interval(self.poll_interval);

        loop {
            interval.tick().await;
        }
    }
}

#[derive(Default)]
pub struct WorkerBuilder {
    concurrency: Option<usize>,
    poll_interval: Option<Duration>,
    jobs: HashMap<String, WorkerFn>,
}

impl WorkerBuilder {
    pub fn build(self) -> Worker {
        Worker {
            concurrency: self.concurrency.unwrap_or_else(num_cpus::get),
            poll_interval: self.poll_interval.unwrap_or(Duration::from_millis(1000)),
            jobs: self.jobs,
        }
    }

    pub fn concurrency(&mut self, value: usize) -> &mut Self {
        self.concurrency = Some(value);
        self
    }

    pub fn poll_interval(&mut self, value: Duration) -> &mut Self {
        self.poll_interval = Some(value);
        self
    }

    pub fn define_job<T, E, Fut, F>(&mut self, identifier: &str, job_fn: F) -> &mut Self
    where
        T: for<'de> Deserialize<'de> + Send,
        E: Debug,
        Fut: Future<Output = Result<(), E>> + Send,
        F: Fn(WorkerContext, T) -> Fut + Send + Sync + 'static,
    {
        let job_fn = Arc::new(job_fn);
        let worker_fn = move |ctx: WorkerContext, payload: Arc<String>| {
            let job_fn = job_fn.clone();
            async move {
                let de_payload = serde_json::from_str(&payload);

                match de_payload {
                    Err(e) => Err(format!("{:?}", e)),
                    Ok(p) => {
                        let job_result = job_fn(ctx, p).await;
                        match job_result {
                            Err(e) => Err(format!("{:?}", e)),
                            Ok(v) => Ok(v),
                        }
                    }
                }
            }
            .boxed()
        };

        self.jobs
            .insert(identifier.to_string(), Box::new(worker_fn));
        self
    }
}
