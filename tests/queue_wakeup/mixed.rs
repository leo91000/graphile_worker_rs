use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use graphile_worker::{IntoTaskHandlerResult, JobSpec, TaskHandler, Worker, WorkerContext};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::helpers::with_test_db;

#[derive(Clone, Debug, Default)]
struct MixedState {
    started: Arc<Notify>,
    finish: Arc<Notify>,
    named_active: Arc<AtomicBool>,
    named_done: Arc<AtomicUsize>,
    unnamed_done: Arc<AtomicUsize>,
}

#[derive(Serialize, Deserialize)]
struct MixedJob {
    named: bool,
    wait: bool,
}

impl TaskHandler for MixedJob {
    const IDENTIFIER: &'static str = "mixed_queue_wakeup";

    /// Holds the first named claim while higher-priority unnamed jobs are added.
    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let state = ctx.get_ext::<MixedState>().unwrap();
        if self.named {
            assert!(!state.named_active.swap(true, Ordering::SeqCst));
        }
        if self.wait {
            state.started.notify_one();
            state.finish.notified().await;
        }
        graphile_worker_runtime::sleep(Duration::from_millis(1)).await;
        if self.named {
            state.named_done.fetch_add(1, Ordering::SeqCst);
            state.named_active.store(false, Ordering::SeqCst);
        } else {
            state.unnamed_done.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// Makes unnamed work consume more than the initial and release-triggered fanout.
/// The first named job's gate establishes insertion order without timing sleeps.
async fn mixed_backlog(batched: bool, concurrency: usize, stop_early: bool) {
    with_test_db(move |db| async move {
        let state = MixedState::default();
        let mut options = Worker::options()
            .database(db.database.clone())
            .concurrency(concurrency)
            .poll_interval(Duration::from_secs(60))
            .use_notification_delivery(false)
            .listen_os_shutdown_signals(false)
            .shutdown_grace_period(Duration::from_secs(5))
            .add_extension(state.clone())
            .define_job::<MixedJob>();
        if batched {
            options = options.complete_job_batch_delay(Duration::from_millis(10));
        }
        let worker = Arc::new(options.init().await.unwrap());
        let utils = worker.create_utils();
        for index in 0..3 {
            utils.add_job(
                MixedJob { named: true, wait: index == 0 },
                JobSpec {
                    queue_name: Some("serial".into()),
                    priority: Some(if index == 0 { -1 } else { 0 }),
                    max_attempts: Some(1),
                    ..Default::default()
                },
            ).await.unwrap();
        }
        let running = worker.clone();
        let run = tokio::task::spawn_local(async move { running.run().await });
        tokio::time::timeout(Duration::from_secs(5), state.started.notified())
            .await.expect("first named job must own the queue before insertion");
        for _ in 0..12 {
            utils.add_job(
                MixedJob { named: false, wait: false },
                JobSpec { priority: Some(-10), max_attempts: Some(1), ..Default::default() },
            ).await.unwrap();
        }
        if stop_early {
            worker.request_shutdown();
            tokio::time::timeout(Duration::from_secs(5), worker.shutdown_signal().clone())
                .await.expect("shutdown must be observed before releasing the handler");
        }
        state.finish.notify_one();
        let progress = if stop_early {
            Ok(())
        } else {
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if state.named_done.load(Ordering::SeqCst) == 3
                        && state.unnamed_done.load(Ordering::SeqCst) == 12
                        && db.get_jobs().await.is_empty()
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await
        };
        worker.request_shutdown();
        tokio::time::timeout(Duration::from_secs(5), run)
            .await.expect("mixed drain must stop and flush outcomes").unwrap().unwrap();
        assert!(progress.is_ok(),
            "mixed backlog stalled: batched={batched}, concurrency={concurrency}, named={}, unnamed={}",
            state.named_done.load(Ordering::SeqCst), state.unnamed_done.load(Ordering::SeqCst));
        let jobs = db.get_jobs().await;
        if stop_early {
            assert_eq!(state.named_done.load(Ordering::SeqCst), 1);
            assert_eq!(state.unnamed_done.load(Ordering::SeqCst), 0);
            assert_eq!(jobs.len(), 14);
            assert!(jobs.iter().all(|job| job.attempts == 0 && job.locked_by.is_none()));
        } else {
            assert!(jobs.is_empty());
        }
        for queue in db.get_job_queues().await {
            assert!(queue.locked_by.is_none() && queue.locked_at.is_none());
        }
    }).await;
}

#[tokio::test]
async fn mixed_backlog_drains_without_an_insert_notification_or_poll() {
    for concurrency in [1, 3] {
        for batched in [false, true] {
            mixed_backlog(batched, concurrency, false).await;
        }
    }
}

#[tokio::test]
async fn shutdown_stops_direct_continuations_before_claiming_the_backlog() {
    for batched in [false, true] {
        mixed_backlog(batched, 1, true).await;
    }
}
