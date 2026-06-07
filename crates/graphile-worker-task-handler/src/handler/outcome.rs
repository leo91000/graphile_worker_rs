use graphile_worker_ctx::WorkerContext;
use serde_json::Value;
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
