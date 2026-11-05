mod helpers;

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, LocalQueueConfig, RefetchDelayConfig, TaskHandler, Worker,
    WorkerContext,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
struct Executions {
    active: Arc<AtomicBool>,
    count: Arc<AtomicUsize>,
}

#[derive(Deserialize, Serialize)]
struct SerialJob {
    fail: bool,
}

impl TaskHandler for SerialJob {
    const IDENTIFIER: &'static str = "queue_wakeup";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let state = ctx.get_ext::<Executions>().unwrap();
        assert!(
            !state.active.swap(true, Ordering::SeqCst),
            "named queue handlers must remain serial"
        );
        graphile_worker_runtime::sleep(Duration::from_millis(1)).await;
        state.count.fetch_add(1, Ordering::SeqCst);
        state.active.store(false, Ordering::SeqCst);
        if self.fail {
            Err("intentional first-job failure")
        } else {
            Ok(())
        }
    }
}

/// Preload a backlog, then disable notifications and choose a polling interval
/// much longer than the progress bound. Local queues also have a long refetch
/// delay: only a persisted queue release should pulse it early.
async fn backlog_drains(batched: bool, failure_attempts: Option<i16>, job_count: usize) {
    for queue_count in [0, 1, 2] {
        helpers::with_test_db(move |db| async move {
            let state = Executions::default();
            let mut options = Worker::options()
                .database(db.database.clone())
                .concurrency(3)
                .poll_interval(Duration::from_secs(60))
                .use_notification_delivery(false)
                .listen_os_shutdown_signals(false)
                .add_extension(state.clone())
                .define_job::<SerialJob>();
            if batched {
                options = options
                    .complete_job_batch_delay(Duration::from_millis(10))
                    .fail_job_batch_delay(Duration::from_millis(10));
            }
            if queue_count > 0 {
                options = options.local_queue(
                    LocalQueueConfig::default()
                        .with_size(5)
                        .with_queue_count(queue_count)
                        .with_refetch_delay(
                            RefetchDelayConfig::default()
                                .with_duration(Duration::from_secs(60))
                                .with_threshold(5)
                                .with_max_abort_threshold(1),
                        ),
                );
            }
            let worker = Arc::new(options.init().await.unwrap());
            let utils = worker.create_utils();
            let mut failed_id = None;
            for index in 0..job_count {
                let fail = index == 0 && failure_attempts.is_some();
                let job = utils
                    .add_job(
                        SerialJob { fail },
                        JobSpec {
                            queue_name: Some("serial".into()),
                            priority: Some(index.try_into().unwrap()),
                            max_attempts: Some(if fail { failure_attempts.unwrap() } else { 1 }),
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap();
                if fail {
                    failed_id = Some(*job.id());
                    if failure_attempts.unwrap() > 1 {
                        // A fourth-attempt backoff exceeds the progress bound,
                        // independent of machine speed. The job is due now.
                        sqlx::query("UPDATE graphile_worker._private_jobs SET attempts=3 WHERE id=$1")
                            .bind(job.id()).execute(&db.test_pool).await.unwrap();
                    }
                }
            }
            let started = std::time::Instant::now();
            let running = worker.clone();
            let run = tokio::task::spawn_local(async move { running.run().await });
            let bound = Duration::from_secs(if job_count > 8 { 30 } else { 5 });
            let progress = tokio::time::timeout(bound, async {
                loop {
                    let jobs = db.get_jobs().await;
                    if state.count.load(Ordering::SeqCst) == job_count
                        && jobs.len() == usize::from(failed_id.is_some())
                        && jobs.iter().all(|job| job.locked_by.is_none())
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await;
            let elapsed = started.elapsed();
            worker.request_shutdown();
            tokio::time::timeout(Duration::from_secs(5), run)
                .await
                .expect("shutdown must ignore pending wakeups")
                .unwrap()
                .unwrap();
            assert!(
                progress.is_ok(),
                "backlog stalled until polling: queues={queue_count}, batched={batched}, failure={failure_attempts:?}, executed={}",
                state.count.load(Ordering::SeqCst)
            );
            eprintln!("QUEUE_DRAIN jobs={job_count} queues={queue_count} batched={batched} failure={failure_attempts:?} elapsed_ms={}", elapsed.as_millis());
            let jobs = db.get_jobs().await;
            if let Some(id) = failed_id {
                assert_eq!(jobs.len(), 1);
                assert_eq!(jobs[0].id, id);
                assert_eq!(jobs[0].attempts, if failure_attempts.unwrap() > 1 { 4 } else { 1 }, "wakeup must not bypass retry backoff");
                assert_eq!(jobs[0].max_attempts, failure_attempts.unwrap());
                assert!(jobs[0].last_error.as_ref().unwrap().contains("intentional"));
                assert_eq!(jobs[0].is_available, failure_attempts.unwrap() > 1);
            } else {
                assert!(jobs.is_empty());
            }
            for queue in db.get_job_queues().await {
                assert!(queue.locked_by.is_none() && queue.locked_at.is_none());
            }
        })
        .await;
    }
}

#[tokio::test]
async fn completion_wakes_named_backlog() {
    backlog_drains(false, None, 8).await;
}

#[tokio::test]
async fn batched_completion_wakes_named_backlog() {
    backlog_drains(true, None, 8).await;
}

#[tokio::test]
async fn retryable_failure_wakes_named_backlog() {
    backlog_drains(false, Some(8), 8).await;
}

#[tokio::test]
async fn batched_retryable_failure_wakes_named_backlog() {
    backlog_drains(true, Some(8), 8).await;
}

#[tokio::test]
async fn permanent_failure_wakes_named_backlog() {
    backlog_drains(false, Some(1), 8).await;
}

#[tokio::test]
async fn batched_permanent_failure_wakes_named_backlog() {
    backlog_drains(true, Some(1), 8).await;
}

/// Manual end-to-end backlog measurement with the same serial-execution,
/// persistence and unlocked-queue assertions as the regressions. Includes a
/// 1ms handler await, three slots, and direct/one/two local queue configurations.
#[tokio::test]
#[ignore = "manual named-queue drain benchmark"]
async fn benchmark_named_queue_drain() {
    backlog_drains(false, None, 200).await;
    backlog_drains(true, None, 200).await;
}

#[path = "queue_wakeup/mixed.rs"]
mod mixed;
