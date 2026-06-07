use graphile_worker_ctx::WorkerContext;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use std::future::Future;

use super::definition::JobDefinition;

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
