#[path = "common/basic_jobs.rs"]
mod basic_jobs;
#[path = "common/distribution_jobs.rs"]
mod distribution_jobs;
#[path = "common/plugins.rs"]
mod plugins;
#[path = "common/refetch_jobs.rs"]
mod refetch_jobs;
#[path = "common/shutdown_jobs.rs"]
mod shutdown_jobs;

use graphile_worker::{
    HookRegistry, IntoTaskHandlerResult, JobSpec, LocalQueueConfig, LocalQueueInit, Plugin,
    RefetchDelayConfig, TaskHandler, Worker, WorkerContext,
};
use graphile_worker_runtime::sleep as runtime_sleep;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    task::spawn_local,
    time::{sleep, Instant},
};

use crate::helpers::{with_test_db, StaticCounter};

use basic_jobs::*;
use distribution_jobs::*;
use plugins::*;
use refetch_jobs::*;
use shutdown_jobs::*;

async fn wait_for_counter(
    counter: &StaticCounter,
    expected: u32,
    timeout: Duration,
    poll_interval: Duration,
    timeout_message: &str,
) {
    let start_time = Instant::now();
    while counter.get().await < expected {
        if start_time.elapsed() > timeout {
            panic!("{timeout_message}, got {}", counter.get().await);
        }
        sleep(poll_interval).await;
    }
}
