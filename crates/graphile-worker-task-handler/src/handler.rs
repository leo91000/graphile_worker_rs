use graphile_worker_ctx::WorkerContext;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Type-erased function used by workers to run a task from a [`WorkerContext`].
pub type TaskHandlerFn = Arc<
    dyn Fn(WorkerContext) -> Pin<Box<dyn Future<Output = TaskHandlerOutcome> + Send>> + Send + Sync,
>;

/// Outcome returned by type-erased task handlers.
///
/// Normal task handlers only produce complete/failed outcomes. Batch handlers
/// may also include a replacement payload so only failed batch items are retried.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskHandlerOutcome {
    Complete,
    Failed {
        error: String,
        replacement_payload: Option<Value>,
    },
}

impl TaskHandlerOutcome {
    pub fn failed(error: impl Into<String>) -> Self {
        Self::Failed {
            error: error.into(),
            replacement_payload: None,
        }
    }

    pub fn failed_with_replacement(
        error: impl Into<String>,
        replacement_payload: impl Into<Value>,
    ) -> Self {
        Self::Failed {
            error: error.into(),
            replacement_payload: Some(replacement_payload.into()),
        }
    }
}

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
            Box::pin(run_task_from_worker_ctx_outcome::<T>(ctx))
                as Pin<Box<dyn Future<Output = TaskHandlerOutcome> + Send>>
        };

        Self {
            identifier: T::IDENTIFIER,
            handler: Arc::new(handler),
        }
    }

    /// Creates a job definition for a batch task handler type.
    pub fn of_batch<T: BatchTaskHandler>() -> Self {
        let handler = move |ctx: WorkerContext| {
            let ctx = ctx.clone();
            Box::pin(run_batch_task_from_worker_ctx::<T>(ctx))
                as Pin<Box<dyn Future<Output = TaskHandlerOutcome> + Send>>
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

/// Result returned by a batch task handler.
///
/// `ItemResults` must have the same length and order as the input batch. Failed
/// item positions are retried with the corresponding original payload values;
/// successful item positions are removed from the retried job payload.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchTaskResult<E> {
    Complete,
    FailAll(E),
    ItemResults(Vec<Result<(), E>>),
}

/// Trait for converting batch task return types into a standardized result.
pub trait IntoBatchTaskHandlerResult {
    fn into_batch_task_handler_result(self) -> BatchTaskResult<impl Debug>;
}

impl IntoBatchTaskHandlerResult for () {
    fn into_batch_task_handler_result(self) -> BatchTaskResult<impl Debug> {
        BatchTaskResult::<()>::Complete
    }
}

impl<D: Debug> IntoBatchTaskHandlerResult for Result<(), D> {
    fn into_batch_task_handler_result(self) -> BatchTaskResult<impl Debug> {
        match self {
            Ok(()) => BatchTaskResult::Complete,
            Err(error) => BatchTaskResult::FailAll(error),
        }
    }
}

impl<D: Debug> IntoBatchTaskHandlerResult for Vec<Result<(), D>> {
    fn into_batch_task_handler_result(self) -> BatchTaskResult<impl Debug> {
        BatchTaskResult::ItemResults(self)
    }
}

impl<D: Debug> IntoBatchTaskHandlerResult for BatchTaskResult<D> {
    fn into_batch_task_handler_result(self) -> BatchTaskResult<impl Debug> {
        self
    }
}

/// Core trait for defining batch task handlers.
///
/// Implement this when a single database job should contain an array of item
/// payloads and the worker should retry only the failed items after partial
/// success. `Self` is the item payload type, not `Vec<Self>`.
pub trait BatchTaskHandler: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {
    /// Unique identifier for this batch task type.
    const IDENTIFIER: &'static str;

    /// Returns a reusable registration value for this batch task handler.
    fn definition() -> JobDefinition
    where
        Self: Sized,
    {
        JobDefinition::of_batch::<Self>()
    }

    /// Execute a batch of item payloads.
    fn run_batch(
        items: Vec<Self>,
        ctx: WorkerContext,
    ) -> impl Future<Output = impl IntoBatchTaskHandlerResult> + Send + 'static;
}

async fn run_task_from_worker_ctx_outcome<T: TaskHandler>(
    worker_context: WorkerContext,
) -> TaskHandlerOutcome {
    match run_task_from_worker_ctx::<T>(worker_context).await {
        Ok(()) => TaskHandlerOutcome::Complete,
        Err(error) => TaskHandlerOutcome::failed(error),
    }
}

pub async fn run_batch_task_from_worker_ctx<T: BatchTaskHandler>(
    worker_context: WorkerContext,
) -> TaskHandlerOutcome {
    let original_payload = worker_context.payload().clone();
    let item_payloads = match original_payload.as_array() {
        Some(items) => items.clone(),
        None => {
            return TaskHandlerOutcome::failed("batch job payload must be a JSON array");
        }
    };

    let items = match Vec::<T>::deserialize(&original_payload) {
        Ok(items) => items,
        Err(error) => return TaskHandlerOutcome::failed(format!("{error:?}")),
    };

    let item_count = items.len();
    let result = T::run_batch(items, worker_context)
        .await
        .into_batch_task_handler_result();

    batch_result_to_task_outcome(result, item_count, item_payloads)
}

fn batch_result_to_task_outcome<E: Debug>(
    result: BatchTaskResult<E>,
    item_count: usize,
    item_payloads: Vec<Value>,
) -> TaskHandlerOutcome {
    match result {
        BatchTaskResult::Complete => TaskHandlerOutcome::Complete,
        BatchTaskResult::FailAll(error) => TaskHandlerOutcome::failed(format!("{error:?}")),
        BatchTaskResult::ItemResults(results) => {
            if results.len() != item_count {
                return TaskHandlerOutcome::failed(format!(
                    "batch handler returned {} results for {item_count} payload items",
                    results.len()
                ));
            }

            let mut failed_items = Vec::new();
            let mut errors = Vec::new();

            for (index, result) in results.into_iter().enumerate() {
                let Err(error) = result else {
                    continue;
                };

                failed_items.push(item_payloads[index].clone());
                errors.push(format!("{index}: {error:?}"));
            }

            if failed_items.is_empty() {
                return TaskHandlerOutcome::Complete;
            }

            TaskHandlerOutcome::failed_with_replacement(
                format!(
                    "{} batch item(s) failed: {}",
                    failed_items.len(),
                    errors.join(", ")
                ),
                Value::Array(failed_items),
            )
        }
    }
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
