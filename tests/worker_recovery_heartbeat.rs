use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker::{
    FailureReason, HookRegistry, IntoTaskHandlerResult, JobRecovery, JobRecoveryResult, JobSpec,
    SweepStaleWorkersOptions, TaskHandler, Worker, WorkerContext, WorkerRecoveryConfig,
    WorkerUtils, INFRASTRUCTURE_RESILIENT_FLAG,
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

#[tokio::test]
async fn background_sweeper_recovers_job_from_dead_worker() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 5 }, JobSpec::default())
            .await
            .expect("failed to add job");

        let worker_a = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_secs(60))
                .sweep_threshold(Duration::from_millis(250))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .define_job::<LongJob>()
                .init()
                .await
                .expect("failed to init worker A"),
        );

        let worker_a_id = worker_a.worker_id().to_string();
        let worker_a_for_run = Arc::clone(&worker_a);
        let worker_a_fut = spawn_local(async move {
            let _ = worker_a_for_run.run().await;
        });

        let start = Instant::now();
        loop {
            let jobs = test_db.get_jobs().await;
            if jobs
                .iter()
                .any(|j| j.id == *job.id() && j.locked_by.as_deref() == Some(&worker_a_id))
            {
                break;
            }

            assert!(
                start.elapsed() <= Duration::from_secs(5),
                "worker A should lock the job before the test timeout"
            );
            sleep(Duration::from_millis(50)).await;
        }

        worker_a_fut.abort();
        let _ = worker_a_fut.await;

        let sweeper = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(1)
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_millis(100))
                .sweep_threshold(Duration::from_millis(250))
                .recovery_delay(Duration::from_millis(100))
                .listen_os_shutdown_signals(false)
                .init()
                .await
                .expect("failed to init sweeper worker"),
        );

        let sweeper_for_run = Arc::clone(&sweeper);
        let sweeper_fut = spawn_local(async move { sweeper_for_run.run().await });

        let start = Instant::now();
        loop {
            let jobs = test_db.get_jobs().await;
            let recovered = jobs
                .iter()
                .find(|j| j.id == *job.id())
                .is_some_and(|j| j.locked_by.is_none() && j.attempts == 0);
            if recovered {
                break;
            }

            if start.elapsed() > Duration::from_secs(8) {
                sweeper.request_shutdown();
                let _ = sweeper_fut.await;
                panic!("background sweeper should recover the dead worker job");
            }

            sleep(Duration::from_millis(100)).await;
        }

        sweeper.request_shutdown();
        sweeper_fut
            .await
            .expect("sweeper task should not panic")
            .expect("sweeper should shut down cleanly");
    })
    .await;
}

#[tokio::test]
async fn resilient_job_uses_extended_sweep_threshold() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let worker_id = "resilient_worker";
        let job = utils
            .add_job(
                LongJob { id: 6 },
                JobSpec::builder()
                    .flags(vec![INFRASTRUCTURE_RESILIENT_FLAG.to_string()])
                    .build(),
            )
            .await
            .expect("failed to add job");

        sqlx::query("SELECT graphile_worker.worker_heartbeat($1::text)")
            .bind(worker_id)
            .execute(&test_db.test_pool)
            .await
            .expect("failed to register resilient worker heartbeat");

        sqlx::query(
            "UPDATE graphile_worker._private_workers SET last_heartbeat_at = now() - interval '2 minutes' WHERE id = $1",
        )
        .bind(worker_id)
        .execute(&test_db.test_pool)
        .await
        .expect("failed to age resilient worker heartbeat");

        sqlx::query(
            r#"
                UPDATE graphile_worker._private_jobs
                    SET attempts = 1,
                        locked_by = $1::text,
                        locked_at = now() - interval '2 minutes'
                    WHERE id = $2::bigint
            "#,
        )
        .bind(worker_id)
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create resilient stale worker lock");

        let config = WorkerRecoveryConfig::default()
            .sweep_threshold(Duration::from_secs(60))
            .resilient_sweep_threshold_multiplier(3);
        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());

        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                recovery_config: Some(config.clone()),
                recovery_delay: Some(Duration::from_millis(100)),
                dry_run: false,
                ..Default::default()
            })
            .await
            .expect("failed to sweep resilient worker before extended threshold");

        assert!(result.worker_ids.is_empty());
        let jobs = test_db.get_jobs().await;
        let still_locked = jobs
            .iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");
        assert_eq!(still_locked.locked_by.as_deref(), Some(worker_id));

        sqlx::query(
            "UPDATE graphile_worker._private_workers SET last_heartbeat_at = now() - interval '4 minutes' WHERE id = $1",
        )
        .bind(worker_id)
        .execute(&test_db.test_pool)
        .await
        .expect("failed to age resilient worker heartbeat");

        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                recovery_config: Some(config),
                recovery_delay: Some(Duration::from_millis(100)),
                dry_run: false,
                ..Default::default()
            })
            .await
            .expect("failed to sweep resilient worker after extended threshold");

        assert_eq!(result.worker_ids, vec![worker_id.to_string()]);
        assert_eq!(result.recovered_count, 1);

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

#[tokio::test]
async fn recovery_hook_skip_leaves_job_locked() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 7 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET attempts = 1, locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("dead_skip_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create skipped recovery lock");

        let mut hooks = HookRegistry::new();
        hooks.on(JobRecovery, |_ctx| async { JobRecoveryResult::Skip });

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
            .expect("failed to sweep skipped recovery lock");

        assert_eq!(result.worker_ids, vec!["dead_skip_worker".to_string()]);
        assert_eq!(result.recovered_count, 0);

        let jobs = test_db.get_jobs().await;
        let skipped = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert_eq!(skipped.locked_by.as_deref(), Some("dead_skip_worker"));
        assert_eq!(skipped.attempts, 1);
    })
    .await;
}

#[tokio::test]
async fn recovery_hook_fail_with_backoff_unlocks_with_retry_delay() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(LongJob { id: 8 }, JobSpec::default())
            .await
            .expect("failed to add job");

        sqlx::query(
            "UPDATE graphile_worker._private_jobs SET attempts = 1, locked_by = $1, locked_at = now() - interval '10 minutes' WHERE id = $2",
        )
        .bind("dead_fail_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create fail-with-backoff recovery lock");

        let before_sweep = chrono::Utc::now();
        let mut hooks = HookRegistry::new();
        hooks.on(JobRecovery, |_ctx| async {
            JobRecoveryResult::FailWithBackoff
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
            .expect("failed to sweep fail-with-backoff recovery lock");

        assert_eq!(result.worker_ids, vec!["dead_fail_worker".to_string()]);
        assert_eq!(result.recovered_count, 1);

        let jobs = test_db.get_jobs().await;
        let failed = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");

        assert!(failed.locked_by.is_none());
        assert_eq!(failed.attempts, 1);
        assert_eq!(failed.last_error.as_deref(), Some("WorkerCrashed"));
        assert!(
            failed.run_at >= before_sweep + chrono::Duration::seconds(1),
            "fail-with-backoff should apply normal retry delay"
        );
    })
    .await;
}

#[tokio::test]
async fn queued_job_recovery_counts_jobs_and_unlocks_queue() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let job = utils
            .add_job(
                LongJob { id: 9 },
                JobSpec::builder()
                    .queue_name("recovery_count_queue")
                    .build(),
            )
            .await
            .expect("failed to add queued job");

        sqlx::query(
            r#"
                UPDATE graphile_worker._private_jobs
                    SET attempts = 1,
                        locked_by = $1::text,
                        locked_at = now() - interval '10 minutes'
                    WHERE id = $2::bigint
            "#,
        )
        .bind("dead_queued_worker")
        .bind(*job.id())
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create queued job recovery lock");

        sqlx::query(
            r#"
                UPDATE graphile_worker._private_job_queues
                    SET locked_by = $1::text,
                        locked_at = now() - interval '10 minutes'
                    WHERE queue_name = 'recovery_count_queue'
            "#,
        )
        .bind("dead_queued_worker")
        .execute(&test_db.test_pool)
        .await
        .expect("failed to create queued queue recovery lock");

        let sweep_utils = WorkerUtils::new(test_db.database.clone(), "graphile_worker".to_string());
        let result = sweep_utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_secs(60)),
                recovery_delay: Some(Duration::from_millis(100)),
                dry_run: false,
                ..Default::default()
            })
            .await
            .expect("failed to sweep queued recovery lock");

        assert_eq!(result.worker_ids, vec!["dead_queued_worker".to_string()]);
        assert_eq!(result.recovered_count, 1);

        let jobs = test_db.get_jobs().await;
        let recovered = jobs
            .into_iter()
            .find(|j| j.id == *job.id())
            .expect("job should exist");
        assert!(recovered.locked_by.is_none());
        assert_eq!(recovered.attempts, 0);

        let queues = test_db.get_job_queues().await;
        let queue = queues
            .into_iter()
            .find(|queue| queue.queue_name == "recovery_count_queue")
            .expect("queue should exist");
        assert!(queue.locked_by.is_none());
        assert!(queue.locked_at.is_none());
    })
    .await;
}
