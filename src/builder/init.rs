use std::sync::Arc;
use std::time::Duration;

use graphile_worker_migrations::migrate;
use rand::Rng;

use crate::batcher::{CompletionBatcher, FailureBatcher};
use crate::Worker;
use graphile_worker_queries::task_identifiers::{get_tasks_details, SharedTaskDetails};

use super::database::connect_default_database;
use super::shutdown::{configured_shutdown_signal, manual_shutdown_signal_pair};
use super::{WorkerBuildError, WorkerOptions};

impl WorkerOptions {
    /// Initializes a worker with the configured options.
    ///
    /// Process:
    /// 1. Establishes a database connection (using the provided pool or creating one from the URL)
    /// 2. Runs database migrations to ensure the schema is up to date
    /// 3. Registers the task handlers
    /// 4. Initializes the worker with a random ID and the configured settings
    ///
    /// # Returns
    /// * `Result<Worker, WorkerBuildError>` - A fully configured worker instance or an error
    ///
    /// # Errors
    /// Can fail if:
    /// * Database connection fails
    /// * Database URL is missing and no pool was provided
    /// * Migrations fail
    /// * Task registration fails
    ///
    /// # Example
    /// ```no_run
    /// # use graphile_worker::WorkerOptions;
    /// # use graphile_worker::{TaskHandler, WorkerContext, IntoTaskHandlerResult};
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Deserialize, Serialize)]
    /// # struct MyTask { data: String }
    /// # impl TaskHandler for MyTask {
    /// #     const IDENTIFIER: &'static str = "my_task";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult { Ok::<(), String>(()) }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let worker = WorkerOptions::default()
    ///     .database_url("postgres://user:password@localhost/mydb")
    ///     .schema("my_app_worker")
    ///     .define_job::<MyTask>()
    ///     .init()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn init(self) -> Result<Worker, WorkerBuildError> {
        let shutdown_config = self.worker_shutdown_config.unwrap_or_default();

        let database = match self.database {
            Some(database) => database,
            None => {
                let db_url = self
                    .database_url
                    .ok_or(WorkerBuildError::MissingDatabaseUrl)?;

                connect_default_database(&db_url, self.max_pg_conn.unwrap_or(20)).await?
            }
        };

        let schema = self.schema.unwrap_or_default();

        migrate(&database, &schema).await?;

        let task_details: SharedTaskDetails =
            get_tasks_details(&database, &schema, self.jobs.keys().cloned().collect())
                .await?
                .into();

        let mut random_bytes = [0u8; 9];
        rand::rng().fill_bytes(&mut random_bytes);

        let (manual_signal, shutdown_notifier) = manual_shutdown_signal_pair();
        let shutdown_signal = configured_shutdown_signal(manual_signal, &shutdown_config);

        let worker_id = format!("graphile_worker_{}", hex::encode(random_bytes));
        let poll_interval = self.poll_interval.unwrap_or(Duration::from_millis(1000));
        let use_notification_delivery = self.use_notification_delivery.unwrap_or(true);

        let hooks = Arc::new(self.hooks);

        let concurrency = self.concurrency.unwrap_or_else(num_cpus::get);

        let local_queue_config = if self.forbidden_flags.is_empty() {
            self.local_queue_config
        } else {
            None
        };
        if let Some(config) = local_queue_config.as_ref() {
            config.validate(poll_interval)?;
        }

        let completion_batcher = self.complete_job_batch_delay.map(|delay| {
            Arc::new(CompletionBatcher::new(
                delay,
                database.clone(),
                schema.clone(),
                worker_id.clone(),
                hooks.clone(),
                shutdown_signal.clone(),
            ))
        });

        let failure_batcher = self.fail_job_batch_delay.map(|delay| {
            Arc::new(FailureBatcher::new(
                delay,
                database.clone(),
                schema.clone(),
                worker_id.clone(),
                hooks.clone(),
                shutdown_signal.clone(),
            ))
        });

        let recovery_config = self.worker_recovery_config.unwrap_or_default();

        Ok(Worker {
            worker_id,
            concurrency,
            poll_interval,
            use_notification_delivery,
            jobs: self.jobs,
            database,
            schema,
            task_details,
            forbidden_flags: self.forbidden_flags,
            crontabs: self.crontabs.unwrap_or_default(),
            use_local_time: self.use_local_time,
            shutdown_signal,
            shutdown_notifier,
            extensions: self.extensions.into(),
            hooks,
            local_queue_config,
            completion_batcher,
            failure_batcher,
            recovery_config,
            shutdown_config,
        })
    }
}
