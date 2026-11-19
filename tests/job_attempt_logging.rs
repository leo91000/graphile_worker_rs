mod helpers;

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use graphile_worker::{IntoTaskHandlerResult, JobSpec, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};
use tracing::subscriber::set_global_default;

#[derive(Clone, Default)]
struct LogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
struct Task {
    fail: bool,
}

impl TaskHandler for Task {
    const IDENTIFIER: &'static str = "job_attempt_logging";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        if self.fail {
            Err("test failure")
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn success_and_failure_log_attempt_limits_as_structured_fields() {
    let log = LogWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer({
            let log = log.clone();
            move || log.clone()
        })
        .finish();
    set_global_default(subscriber).unwrap();

    helpers::with_test_db(|db| async move {
        let worker = db
            .create_worker_options()
            .define_job::<Task>()
            .init()
            .await
            .unwrap();

        let spec = JobSpec {
            max_attempts: Some(3),
            ..Default::default()
        };
        let utils = worker.create_utils();
        utils
            .add_job(Task { fail: false }, spec.clone())
            .await
            .unwrap();
        utils.add_job(Task { fail: true }, spec).await.unwrap();
        worker.run_once().await.unwrap();
    })
    .await;

    let logs = log.0.lock().unwrap();
    let messages = std::str::from_utf8(&logs).unwrap();
    for message in ["Completed task with success", "Failed task"] {
        let line = messages
            .lines()
            .find(|line| line.contains(message))
            .unwrap();
        assert!(line.contains("attempts=1 max_attempts=3"), "{line}");
    }
}
