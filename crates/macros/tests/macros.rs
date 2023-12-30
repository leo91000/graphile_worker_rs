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
    let identifier = test_fn::identifier();

    assert_eq!("test_fn", identifier);
}
