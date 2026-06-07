#![allow(dead_code)]

use graphile_worker::{IntoTaskHandlerResult, JobSpecBuilder, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;

mod helpers;

#[path = "fail_job/error.rs"]
mod error;
#[path = "fail_job/panic.rs"]
mod panic;
#[path = "fail_job/permanent.rs"]
mod permanent;
#[path = "fail_job/retry_delay.rs"]
mod retry_delay;
