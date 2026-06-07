use graphile_worker_ctx::WorkerContext;
use serde::Deserialize;
use serde_json::Value;
use std::fmt::Debug;

use super::batch::{BatchTaskHandler, BatchTaskResult, IntoBatchTaskHandlerResult};
use super::outcome::TaskHandlerOutcome;
use super::task::{IntoTaskHandlerResult, TaskHandler};

pub(in crate::handler) async fn run_task_from_worker_ctx_outcome<T: TaskHandler>(
    worker_context: WorkerContext,
) -> TaskHandlerOutcome {
    match run_task_from_worker_ctx::<T>(worker_context).await {
        Ok(()) => TaskHandlerOutcome::Complete,
        Err(error) => TaskHandlerOutcome::failed(error),
    }
}

pub(in crate::handler) async fn run_batch_task_from_worker_ctx<T: BatchTaskHandler>(
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
    let job = T::deserialize(worker_context.payload());
    let Ok(job) = job else {
        let e = job.err().unwrap();
        return Err(format!("{e:?}"));
    };

    job.run(worker_context)
        .await
        .into_task_handler_result()
        .map_err(|e| format!("{e:?}"))
}
