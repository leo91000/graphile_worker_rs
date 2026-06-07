#[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
use std::time::Duration;

use crate::{spawn, JoinError};

#[cfg(feature = "runtime-async-std")]
use futures::executor::block_on;

#[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
use crate::sleep;

#[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
#[test]
fn tokio_spawn_completes_aborts_and_reports_panics() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();

    runtime.block_on(async {
        assert_eq!(spawn(async { 42 }).await.unwrap(), 42);

        let handle = spawn(async {
            sleep(Duration::from_secs(60)).await;
            7
        });
        handle.abort_handle().abort();
        assert!(matches!(handle.await, Err(JoinError::Aborted)));

        let result = spawn(async {
            panic!("spawn panic");
        })
        .await;

        match result {
            Err(JoinError::Failed(message)) => assert!(message.contains("spawn panic")),
            other => panic!("expected failed join error, got {other:?}"),
        }
    });
}

#[cfg(feature = "runtime-async-std")]
#[test]
fn async_std_spawn_maps_task_panics_to_join_error() {
    block_on(async {
        let result = spawn(async {
            panic!("boom");
        })
        .await;

        match result {
            Err(JoinError::Failed(message)) => assert!(message.contains("boom")),
            other => panic!("expected failed join error, got {other:?}"),
        }
    });
}

#[cfg(all(feature = "runtime-tokio", not(feature = "runtime-async-std")))]
#[test]
#[should_panic(expected = "Tokio runtime")]
fn tokio_only_spawn_requires_tokio_runtime() {
    drop(spawn(async {}));
}
