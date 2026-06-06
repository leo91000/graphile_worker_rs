use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker::{
    FailureReason, HookRegistry, IntoTaskHandlerResult, JobRecovery, JobRecoveryResult, JobSpec,
    SweepStaleWorkersOptions, TaskHandler, Worker, WorkerContext, WorkerUtils,
};
use graphile_worker_runtime::sleep as runtime_sleep;
use serde::{Deserialize, Serialize};
use tokio::{
    task::spawn_local,
    time::{sleep, Instant},
};

mod helpers;

use helpers::with_test_db;

#[derive(Deserialize, Serialize)]
struct LongJob {
    id: i64,
}

impl TaskHandler for LongJob {
    const IDENTIFIER: &'static str = "long_heartbeat_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        runtime_sleep(Duration::from_secs(120)).await;
        Ok::<(), String>(())
    }
}

#[tokio::test]
async fn stale_worker_heartbeat_recovers_locked_jobs() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        utils
            .add_job(LongJob { id: 1 }, JobSpec::default())
            .await
            .expect("failed to add job");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_millis(200))
                .sweep_threshold(Duration::from_millis(300))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .define_job::<LongJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );

        let worker_id = worker.worker_id().to_string();
        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move {
            let _ = worker_for_run.run().await;
        });

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.locked_by.as_deref() == Some(worker_id.as_str()))
            {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        worker_fut.abort();
        let _ = worker_fut.await;

        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
        let sweep_start = Instant::now();
        let mut recovered = false;
        while sweep_start.elapsed() < Duration::from_secs(10) {
            let result = sweep_utils
                .sweep_stale_workers(SweepStaleWorkersOptions {
                    sweep_threshold: Some(Duration::from_millis(300)),
                    recovery_delay: Some(Duration::from_millis(100)),
                    dry_run: false,
                    ..Default::default()
                })
                .await
                .expect("failed to sweep stale workers");

            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.locked_by.is_none() && j.attempts == 0)
            {
                recovered =
                    !result.worker_ids.is_empty() || jobs.iter().any(|j| j.locked_by.is_none());
                if recovered {
                    break;
                }
            }

            sleep(Duration::from_millis(250)).await;
        }

        assert!(recovered, "job should be recovered from stale worker");
    })
    .await;
}

#[tokio::test]
async fn orphan_lock_is_recovered_after_threshold() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 2 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("graphile_worker_orphan")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create orphan lock");

        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
        let before_sweep = chrono::Utc::now();
        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_secs(60)),
                recovery_delay: Some(Duration::from_millis(750)),
                dry_run: false,
                ..Default::default()
            })
            .await
            .expect("failed to sweep orphan lock");

        assert!(result.worker_ids.contains(&"graphile_worker_orphan".to_string()));

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert!(recovered.locked_by.is_none());
        assert_eq!(recovered.attempts, 0);
        assert!(
            recovered.run_at >= before_sweep + chrono::Duration::milliseconds(500),
            "subsecond recovery delay should not be truncated"
        );
    })
    .await;
}

#[tokio::test]
async fn default_recovery_config_does_not_register_worker() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .listen_os_shutdown_signals(false)
                .define_job::<LongJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );

        let worker_for_run = Arc::clone(&worker);
        let worker_fut = spawn_local(async move { worker_for_run.run().await });

        sleep(Duration::from_millis(250)).await;

        let registered_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM graphile_worker._private_workers")
                .fetch_one(&test_db.test_pool)
                .await
                .expect("failed to count registered workers");

        worker.request_shutdown();
        worker_fut
            .await
            .expect("worker task should not panic")
            .expect("worker should shut down cleanly");

        assert_eq!(registered_count, 0);
    })
    .await;
}

#[tokio::test]
async fn recovery_hook_is_applied_to_dead_worker_sweep() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 3 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET attempts = 1, locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("dead_hook_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create orphan lock");

        let calls = Arc::new(AtomicU32::new(0));
        let target_run_at = chrono::Utc::now() + chrono::Duration::seconds(5);
        let mut hooks = HookRegistry::new();
        let hook_calls = Arc::clone(&calls);
        hooks.on(JobRecovery, move |ctx| {
            let hook_calls = Arc::clone(&hook_calls);
            async move {
                assert_eq!(ctx.previous_worker_id, "dead_hook_worker");
                assert_eq!(ctx.reason, FailureReason::WorkerCrashed);
                hook_calls.fetch_add(1, Ordering::SeqCst);
                JobRecoveryResult::Reschedule {
                    run_at: target_run_at,
                    attempts: Some(3),
                }
            }
        });

        let sweep_utils =
            WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string())
                .with_hooks(Arc::new(hooks));
        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_secs(60)),
                recovery_delay: Some(Duration::from_millis(100)),
                dry_run: false,
                ..Default::default()
            })
            .await
            .expect("failed to sweep stale workers");

        assert_eq!(result.recovered_count, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert!(recovered.locked_by.is_none());
        assert_eq!(recovered.attempts, 3);
        let run_at_delta_ms = (recovered.run_at - target_run_at).num_milliseconds().abs();
        assert!(
            run_at_delta_ms <= 1,
            "hook reschedule run_at should be preserved"
        );
    })
    .await;
}

#[tokio::test]
async fn concurrent_sweeps_recover_locked_job_once() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 4 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET attempts = 1, locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("dead_concurrent_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create orphan lock");

        let sweep_options = SweepStaleWorkersOptions {
            sweep_threshold: Some(Duration::from_secs(60)),
            recovery_delay: Some(Duration::from_millis(100)),
            dry_run: false,
            ..Default::default()
        };
        let first_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
        let second_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());

        let (first, second) = tokio::join!(
            first_utils.sweep_stale_workers(sweep_options.clone()),
            second_utils.sweep_stale_workers(sweep_options)
        );

        let first = first.expect("first sweep should succeed");
        let second = second.expect("second sweep should succeed");
        assert_eq!(first.recovered_count + second.recovered_count, 1);

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert!(recovered.locked_by.is_none());
        assert_eq!(recovered.attempts, 0);
    })
    .await;
}
