use super::*;

use std::sync::Mutex;
use tokio::sync::Notify;
use tokio::time::timeout;

#[derive(Clone, Debug, Default)]
struct HeartbeatJobRuns {
    started: Arc<Mutex<Vec<i64>>>,
    release: Arc<Notify>,
}

#[derive(Deserialize, Serialize)]
struct HeldHeartbeatJob;

impl TaskHandler for HeldHeartbeatJob {
    const IDENTIFIER: &'static str = "held_heartbeat_job";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let runs = ctx.get_ext::<HeartbeatJobRuns>().expect("runs extension");
        runs.started.lock().unwrap().push(*ctx.job().id());
        runs.release.notified().await;
        Ok::<(), String>(())
    }
}

#[tokio::test]
async fn failed_heartbeat_retries_without_recovering_a_running_queued_job() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");
        let runs = HeartbeatJobRuns::default();
        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_millis(50))
                .heartbeat_interval(Duration::from_millis(100))
                .sweep_interval(Duration::from_millis(100))
                .sweep_threshold(Duration::from_secs(1))
                .recovery_delay(Duration::ZERO)
                .listen_os_shutdown_signals(false)
                .add_extension(runs.clone())
                .define_job::<HeldHeartbeatJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );
        let worker_for_run = Arc::clone(&worker);
        let handle = tokio::spawn(async move { worker_for_run.run().await });

        // Registration has no metadata; wait for a successful background heartbeat
        // before injecting a failure so startup itself is not affected.
        timeout(Duration::from_secs(5), async {
            loop {
                let ready: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM graphile_worker._private_workers WHERE id = $1 AND metadata IS NOT NULL)",
                )
                .bind(worker.worker_id())
                .fetch_one(&test_db.test_pool)
                .await
                .expect("failed to check initial heartbeat");
                if ready {
                    break;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("background heartbeat should start");

        // Sequence increments survive a rolled-back statement. A trigger injects
        // exactly one failure without replacing the production heartbeat SQL.
        sqlx::raw_sql(
            r#"
            CREATE SEQUENCE graphile_worker.heartbeat_calls;
            CREATE FUNCTION graphile_worker.fail_first_heartbeat()
            RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN
                IF nextval('graphile_worker.heartbeat_calls') = 1 THEN
                    RAISE EXCEPTION 'injected heartbeat failure';
                END IF;
                RETURN NEW;
            END;
            $$;
            CREATE TRIGGER fail_first_heartbeat BEFORE INSERT
                ON graphile_worker._private_workers
                FOR EACH ROW EXECUTE FUNCTION graphile_worker.fail_first_heartbeat();
            "#,
        )
        .execute(&test_db.test_pool)
        .await
        .expect("failed to inject heartbeat failure");

        timeout(Duration::from_secs(5), async {
            loop {
                let called: bool = sqlx::query_scalar(
                    "SELECT is_called FROM graphile_worker.heartbeat_calls",
                )
                .fetch_one(&test_db.test_pool)
                .await
                .expect("failed to check injected failure");
                if called {
                    break;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("injected heartbeat failure should occur");

        let job = utils
            .add_job(
                HeldHeartbeatJob,
                JobSpec::builder().queue_name("heartbeat_retry_queue").build(),
            )
            .await
            .expect("failed to add job");
        timeout(Duration::from_secs(5), async {
            while runs.started.lock().unwrap().is_empty() {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("job should start");

        // Keep the handler running across multiple stale thresholds. A dead
        // heartbeat loop lets this worker's own sweeper run the same job twice.
        sleep(Duration::from_millis(2500)).await;
        let heartbeat_calls: i64 = sqlx::query_scalar(
            "SELECT last_value FROM graphile_worker.heartbeat_calls",
        )
        .fetch_one(&test_db.test_pool)
        .await
        .expect("failed to count heartbeats");
        let registered: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM graphile_worker._private_workers WHERE id = $1)",
        )
        .bind(worker.worker_id())
        .fetch_one(&test_db.test_pool)
        .await
        .expect("failed to check worker registration");
        let started = runs.started.lock().unwrap().clone();
        let still_running = !handle.is_finished();
        let jobs = test_db.get_jobs().await;

        worker.request_shutdown();
        runs.release.notify_waiters();
        timeout(Duration::from_secs(5), handle)
            .await
            .expect("worker should shut down promptly")
            .expect("worker task should not panic")
            .expect("worker should shut down cleanly");

        assert!(heartbeat_calls > 1, "heartbeat should retry after failure");
        assert!(registered, "live worker registration should remain present");
        assert!(still_running, "worker should continue running after failure");
        assert_eq!(started, vec![*job.id()], "job should execute exactly once");
        let stored_job = jobs.iter().find(|j| j.id == *job.id()).expect("running job");
        assert_eq!(stored_job.locked_by.as_deref(), Some(worker.worker_id().as_str()));
        assert!(stored_job.last_error.is_none(), "running job should not be recovered");
    })
    .await;
}
