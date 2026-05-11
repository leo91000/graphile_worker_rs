use graphile_worker_ctx::WorkerContext;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Type-erased function used by workers to run a task from a [`WorkerContext`].
pub type TaskHandlerFn = Arc<
    dyn Fn(WorkerContext) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync,
>;

/// Trait for converting task handler return types into a standardized Result.
///
/// This trait allows task handlers to return different types while maintaining
/// a consistent interface. It enables task handlers to return:
/// - `()` (unit type) for successful execution with no return value
/// - `Result<(), E>` for success/failure with error type E
///
/// The trait converts these different return types into a standardized `Result<(), impl Debug>`.
pub trait IntoTaskHandlerResult {
    /// Converts the implementing type into a task handler result.
    ///
    /// # Returns
    /// A Result<(), impl Debug> where:
    /// - Ok(()) represents successful task execution
    /// - Err(e) represents task failure with a debug-printable error
    fn into_task_handler_result(self) -> Result<(), impl Debug>;
}

/// Reusable registration value for a [`TaskHandler`].
///
/// `JobDefinition` lets crates and modules expose the jobs they provide as
/// values, so applications can register a collection of jobs in one call.
#[derive(Clone)]
pub struct JobDefinition {
    identifier: &'static str,
    handler: TaskHandlerFn,
}

impl JobDefinition {
    /// Creates a job definition for a task handler type.
    pub fn of<T: TaskHandler>() -> Self {
        let handler = move |ctx: WorkerContext| {
            let ctx = ctx.clone();
            Box::pin(run_task_from_worker_ctx::<T>(ctx))
                as Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
        };

        Self {
            identifier: T::IDENTIFIER,
            handler: Arc::new(handler),
        }
    }

    /// The identifier handled by this definition.
    pub fn identifier(&self) -> &'static str {
        self.identifier
    }

    /// The type-erased task handler function.
    pub fn handler(&self) -> TaskHandlerFn {
        self.handler.clone()
    }

    /// Splits this definition into the identifier and handler function.
    pub fn into_parts(self) -> (&'static str, TaskHandlerFn) {
        (self.identifier, self.handler)
    }
}

/// Implementation for the unit type, allowing tasks to simply return `()`.
impl IntoTaskHandlerResult for () {
    fn into_task_handler_result(self) -> Result<(), impl Debug> {
        Ok::<_, ()>(())
    }
}

/// Implementation for Result types, allowing tasks to return errors directly.
impl<D: Debug> IntoTaskHandlerResult for Result<(), D> {
    fn into_task_handler_result(self) -> Result<(), impl Debug> {
        self
    }
}

/// Core trait for defining task handlers in Graphile Worker.
///
/// A TaskHandler represents a specific job type that can be processed by Graphile Worker.
/// It defines:
/// - A unique identifier for the task type
/// - The payload structure (via the implementing type's fields)
/// - The execution logic in the `run` method
///
/// # Type Requirements
/// - `Serialize`: The task must be serializable to JSON for storage in the database
/// - `Deserialize`: The task must be deserializable from JSON when retrieved from the database
/// - `Send + Sync + 'static`: The task must be safe to send between threads
///
/// # Example
/// ```
/// use graphile_worker_task_handler::{TaskHandler, IntoTaskHandlerResult};
/// use graphile_worker_ctx::WorkerContext;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize, Serialize)]
/// struct SendEmail {
///     to: String,
///     subject: String,
///     body: String,
/// }
///
/// impl TaskHandler for SendEmail {
///     const IDENTIFIER: &'static str = "send_email";
///
///     async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
///         println!("Sending email to {} with subject '{}'", self.to, self.subject);
///         // Actual email sending logic would go here
///         Ok::<(), String>(())
///     }
/// }
/// ```
pub trait TaskHandler: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {
    /// Unique identifier for this task type.
    ///
    /// This identifier must be unique across all task types in your application.
    /// It is used to match incoming jobs to the correct handler.
    ///
    /// # Best Practices
    /// - Use lowercase snake_case names
    /// - Make names descriptive but concise
    /// - Ensure they are globally unique across your application
    const IDENTIFIER: &'static str;

    /// Returns a reusable registration value for this task handler.
    ///
    /// This is equivalent to [`JobDefinition::of`] and is useful when
    /// a module wants to expose all of its jobs as a collection.
    fn definition() -> JobDefinition
    where
        Self: Sized,
    {
        JobDefinition::of::<Self>()
    }

    /// Execute the task logic.
    ///
    /// This method is called when a job of this task type is processed by the worker.
    /// It contains the actual business logic for processing the job.
    ///
    /// # Arguments
    /// * `self` - The task payload, deserialized from the job's JSON payload
    /// * `ctx` - Worker context providing access to job metadata and extensions
    ///
    /// # Returns
    /// An async result that converts to a TaskHandlerResult, indicating success or failure.
    /// - Return `Ok(())` or just `()` for success
    /// - Return `Err(error)` for failure, which will trigger retries
    fn run(
        self,
        ctx: WorkerContext,
    ) -> impl Future<Output = impl IntoTaskHandlerResult> + Send + 'static;
}

/// Internal function to execute a task handler from a worker context.
///
/// This function:
/// 1. Deserializes the job payload into the specified task handler type
/// 2. Calls the task handler's run method
/// 3. Processes the result
///
/// # Arguments
/// * `worker_context` - The context containing job information and payload
///
/// # Returns
/// * `Result<(), String>` - Ok(()) on success, Err with error message on failure
///
/// # Type Parameters
/// * `T` - The task handler type to deserialize and execute
pub async fn run_task_from_worker_ctx<T: TaskHandler>(
    worker_context: WorkerContext,
) -> Result<(), String> {
    // Deserialize the job payload into the task handler type
    let job = T::deserialize(worker_context.payload());
    let Ok(job) = job else {
        let e = job.err().unwrap();
        return Err(format!("{e:?}"));
    };

    // Execute the task and convert the result
    job.run(worker_context)
        .await
        .into_task_handler_result()
        .map_err(|e| format!("{e:?}"))
}
