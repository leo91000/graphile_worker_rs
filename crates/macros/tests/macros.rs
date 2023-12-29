extern crate archimedes_macros;

use archimedes_task_handler::{SpawnTaskResult, TaskDefinition, TaskHandler};
use tokio_util::sync::CancellationToken;

// Define a function using your macro

#[archimedes_macros::task]
async fn test_fn(payload: String, ctx: String) -> Result<(), String> {
    println!("{} {}", ctx, payload);
    Ok(())
}

#[tokio::test]
async fn test_sample_task() {
    let runner = archimedes_task_handler::TaskDefinition::get_task_runner(&test_fn);

    // You can test the behavior here, for example, by checking the output of the handler
    let result = runner
        .spawn_task(
            "payload".to_string(),
            "context".to_string(),
            CancellationToken::new(),
        )
        .await;

    assert_eq!(Ok(()), result.result);
    assert!(result.duration > std::time::Duration::from_millis(0));
}
