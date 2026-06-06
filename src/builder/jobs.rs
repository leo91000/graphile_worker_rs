use graphile_worker_task_handler::{BatchTaskHandler, JobDefinition, TaskHandler};

use super::WorkerOptions;

impl WorkerOptions {
    /// Registers a task handler type with the worker.
    ///
    /// Primary way to define what types of jobs this worker can process.
    /// Each task handler implements the `TaskHandler` trait, which defines how to
    /// deserialize and run jobs of a specific type.
    ///
    /// # Type Parameters
    /// * `T` - A type implementing the TaskHandler trait
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{WorkerOptions, TaskHandler, WorkerContext, IntoTaskHandlerResult};
    /// # use serde::{Deserialize, Serialize};
    /// #
    /// # #[derive(Deserialize, Serialize)]
    /// # struct SendEmail { to: String }
    /// #
    /// # impl TaskHandler for SendEmail {
    /// #     const IDENTIFIER: &'static str = "send_email";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
    /// #         Ok::<(), String>(())
    /// #     }
    /// # }
    ///
    /// let options = WorkerOptions::default()
    ///     .define_job::<SendEmail>();
    /// ```
    pub fn define_job<T: TaskHandler>(self) -> Self {
        self.define_jobs([T::definition()])
    }

    /// Registers a batch task handler type with the worker.
    ///
    /// Batch handlers receive a vector of item payloads from a single JSON-array
    /// job payload and may return per-item results so successful items are not
    /// retried after partial failure.
    pub fn define_batch_job<T: BatchTaskHandler>(self) -> Self {
        self.define_jobs([T::definition()])
    }

    /// Registers task handler definitions with the worker.
    ///
    /// This is useful when a module or crate exposes the jobs it can process as
    /// reusable values.
    ///
    /// # Arguments
    /// * `jobs` - Job definitions created with [`TaskHandler::definition`] or
    ///   [`JobDefinition::of`]
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::{
    /// #     IntoTaskHandlerResult, JobDefinition, TaskHandler, WorkerContext, WorkerOptions,
    /// # };
    /// # use serde::{Deserialize, Serialize};
    /// #
    /// # #[derive(Deserialize, Serialize)]
    /// # struct SendEmail;
    /// #
    /// # impl TaskHandler for SendEmail {
    /// #     const IDENTIFIER: &'static str = "send_email";
    /// #     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
    /// # }
    /// #
    /// fn jobs() -> [JobDefinition; 1] {
    ///     [SendEmail::definition()]
    /// }
    ///
    /// let options = WorkerOptions::default()
    ///     .define_jobs(jobs());
    /// ```
    pub fn define_jobs<I>(mut self, jobs: I) -> Self
    where
        I: IntoIterator<Item = JobDefinition>,
    {
        for job in jobs {
            let (identifier, worker_fn) = job.into_parts();
            self.jobs.insert(identifier.to_string(), worker_fn);
        }

        self
    }

    /// Adds a flag to the list of forbidden flags.
    ///
    /// Jobs with any forbidden flag will be skipped by this worker instance.
    /// Can be used to implement specialized workers that only handle
    /// certain types of jobs.
    ///
    /// # Arguments
    /// * `flag` - The flag to forbid
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::WorkerOptions;
    ///
    /// // This worker will skip jobs with the "high_memory" flag
    /// let options = WorkerOptions::default()
    ///     .add_forbidden_flag("high_memory");
    /// ```
    pub fn add_forbidden_flag(mut self, flag: &str) -> Self {
        self.forbidden_flags.push(flag.into());
        self
    }
}
