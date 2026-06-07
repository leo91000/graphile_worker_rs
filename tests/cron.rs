use std::sync::Arc;

use chrono::{Duration, DurationRound, Local};
use futures::FutureExt;
use graphile_worker::parse_crontab;
use graphile_worker::HookRegistry;
use graphile_worker::IntoTaskHandlerResult;
use graphile_worker::Worker;
use graphile_worker_crontab_runner::{CronRunner, MockClock};
use graphile_worker_shutdown_signal::ShutdownSignal;
use serde::{Deserialize, Serialize};
use sqlx::query;
use tokio::sync::Notify;
use tokio::{task::spawn_local, time::Instant};

use crate::helpers::{with_test_db, StaticCounter};
use graphile_worker::{TaskHandler, WorkerContext};

mod helpers;

fn create_shutdown_signal() -> (ShutdownSignal, Arc<Notify>) {
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();
    let signal: ShutdownSignal = async move {
        notify_clone.notified().await;
    }
    .boxed()
    .shared();
    (signal, notify)
}

#[path = "cron/backfill.rs"]
mod backfill;
#[path = "cron/registration.rs"]
mod registration;
#[path = "cron/runner.rs"]
mod runner;
