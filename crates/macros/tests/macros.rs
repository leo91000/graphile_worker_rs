use archimedes_task_handler::TaskDefinition;

extern crate archimedes_macros;

// Define a function using your macro

#[archimedes_macros::task(
    identifier = "my_worker_task",
    source_crate = "archimedes_task_handler"
)]
async fn test_fn(payload: String, ctx: String) -> Result<(), String> {
    println!("{} {}", ctx, payload);
    Ok(())
}

fn get_identifier<T: TaskDefinition<String>>() -> &'static str {
    T::identifier()
}

#[tokio::test]
async fn test_sample_task() {
    let identifier = get_identifier::<test_fn>();

    assert_eq!("my_worker_task", identifier);
}
