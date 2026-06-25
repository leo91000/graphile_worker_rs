mod run;

use super::Worker;
use crate::builder::WorkerOptions;
use graphile_worker_database::Schema;
use graphile_worker_utils::WorkerUtils;

impl Worker {
    /// Creates a new `WorkerOptions` builder with default settings.
    ///
    /// This is the starting point for configuring and creating a new worker.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use graphile_worker::Worker;
    /// use std::time::Duration;
    /// use graphile_worker_ctx::WorkerContext;
    /// use serde::{Deserialize, Serialize};
    /// use graphile_worker::{TaskHandler, IntoTaskHandlerResult};
    ///
    /// #[derive(Deserialize, Serialize)]
    /// struct MyTask;
    ///
    /// impl TaskHandler for MyTask {
    ///     const IDENTIFIER: &'static str = "task_name";
    ///     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
    ///         Ok::<(), String>(())
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let worker = Worker::options()
    ///     .concurrency(5)
    ///     .poll_interval(Duration::from_secs(1))
    ///     .database_url("postgres://user:password@localhost/mydb")
    ///     .define_job::<MyTask>()
    ///     .init()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn options() -> WorkerOptions {
        WorkerOptions::default()
    }

    /// Returns the underlying SQLx PostgreSQL pool when the worker uses the SQLx driver.
    #[cfg(feature = "driver-sqlx")]
    pub fn try_pg_pool(&self) -> Option<&sqlx::PgPool> {
        self.database
            .downcast_ref::<graphile_worker_database::sqlx::SqlxDatabase>()
            .map(|database| database.pool())
    }

    /// Returns the underlying SQLx PostgreSQL pool.
    ///
    /// # Panics
    /// Panics if the worker was configured with a non-SQLx database driver.
    #[cfg(feature = "driver-sqlx")]
    pub fn pg_pool(&self) -> &sqlx::PgPool {
        self.try_pg_pool()
            .expect("Worker does not use the SQLx database driver")
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub(crate) fn clone_for_recovery(&self) -> Self {
        Self {
            worker_id: self.worker_id.clone(),
            concurrency: self.concurrency,
            poll_interval: self.poll_interval,
            use_notification_delivery: self.use_notification_delivery,
            jobs: self.jobs.clone(),
            database: self.database.clone(),
            schema: self.schema.clone(),
            task_details: self.task_details.clone(),
            forbidden_flags: self.forbidden_flags.clone(),
            crontabs: self.crontabs.clone(),
            use_local_time: self.use_local_time,
            shutdown_signal: self.shutdown_signal.clone(),
            shutdown_notifier: self.shutdown_notifier.clone(),
            extensions: self.extensions.clone(),
            hooks: self.hooks.clone(),
            local_queue_config: self.local_queue_config.clone(),
            completion_batcher: self.completion_batcher.clone(),
            failure_batcher: self.failure_batcher.clone(),
            recovery_config: self.recovery_config.clone(),
            shutdown_config: self.shutdown_config.clone(),
        }
    }

    /// Creates a utils object for performing worker-related operations.
    pub fn create_utils(&self) -> WorkerUtils {
        let utils = WorkerUtils::new(self.database.clone(), self.schema.clone())
            .with_task_details(self.task_details.clone());

        if self.hooks.is_empty() {
            utils
        } else {
            utils.with_hooks(self.hooks.clone())
        }
    }

    /// Requests a graceful shutdown of the worker.
    pub fn request_shutdown(&self) {
        self.shutdown_notifier.notify_one();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Safety net for a Worker that is constructed and dropped without request_shutdown being called.
        self.shutdown_notifier.notify_one();
    }
}
