use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;

use graphile_worker::{
    AfterJobRun, BeforeJobRun, BeforeJobSchedule, HookRegistry, HookResult, IntoTaskHandlerResult,
    JobComplete, JobFail, JobFetch, JobPermanentlyFail, JobScheduleResult, JobSpec, JobStart,
    LocalQueueConfig, LocalQueueGetJobsComplete, LocalQueueInit, LocalQueueMode,
    LocalQueueRefetchDelayExpired, LocalQueueRefetchDelayStart, LocalQueueReturnJobs,
    LocalQueueSetMode, Plugin, RefetchDelayConfig, TaskHandler, Worker, WorkerContext, WorkerInit,
    WorkerShutdown, WorkerStart,
};
use graphile_worker_runtime::sleep as runtime_sleep;
use serde::{Deserialize, Serialize};
use tokio::{
    task::spawn_local,
    time::{sleep, Duration, Instant},
};

use crate::helpers::with_test_db;

include!("common/run_support.rs");
include!("common/schedule_support.rs");
include!("common/extended_run_support.rs");
include!("common/local_queue_support.rs");
