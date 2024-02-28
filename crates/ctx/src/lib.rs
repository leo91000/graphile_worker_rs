use getset::Getters;
use graphile_worker_job::Job;
use sqlx::PgPool;
use std::sync::Arc;

pub struct AppState<AS = ()>(pub Arc<AS>);

impl<AS> AppState<AS> {
    pub fn new(app_state: AS) -> Self {
        AppState(Arc::new(app_state))
    }
}

impl<AS> Clone for AppState<AS> {
    fn clone(&self) -> Self {
        AppState(self.0.clone())
    }
}

#[derive(Getters)]
#[getset(get = "pub")]
pub struct WorkerContext<AS = ()> {
    pg_pool: PgPool,
    job: Job,
    worker_id: String,
    app_state: AppState<AS>,
}

impl<AS> WorkerContext<AS> {
    pub fn new(pg_pool: PgPool, job: Job, worker_id: String, app_state: AppState<AS>) -> Self {
        WorkerContext {
            pg_pool,
            job,
            worker_id,
            app_state,
        }
    }
}

impl<AS> Clone for WorkerContext<AS> {
    fn clone(&self) -> Self {
        WorkerContext {
            pg_pool: self.pg_pool.clone(),
            job: self.job.clone(),
            worker_id: self.worker_id.clone(),
            app_state: self.app_state.clone(),
        }
    }
}
