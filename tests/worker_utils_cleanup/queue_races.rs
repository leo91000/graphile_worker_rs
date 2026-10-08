use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker::{
    worker_utils::types::CleanupTask, IntoTaskHandlerResult, JobSpec, TaskHandler, Worker,
    WorkerContext,
};
use indoc::formatdoc;
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, timeout};

use crate::helpers::{with_test_db, TestDatabase};

use super::race_database;

fn race_utils(test_db: &TestDatabase) -> graphile_worker::worker_utils::WorkerUtils {
    graphile_worker::worker_utils::WorkerUtils::new(
        graphile_worker_database::Database::new(race_database::RaceDatabase::new(
            test_db.database.clone(),
        )),
        "graphile_worker",
    )
}

#[derive(Clone, Debug, Default)]
struct Executions(Arc<AtomicU32>);

#[derive(Clone, Deserialize, Serialize)]
struct QueueGcJob {
    pause: bool,
}

impl TaskHandler for QueueGcJob {
    const IDENTIFIER: &'static str = "queue_gc_job";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        ctx.get_ext::<Executions>()
            .expect("execution counter")
            .0
            .fetch_add(1, Ordering::SeqCst);
        Ok::<(), String>(())
    }
}

async fn wait_for_lock(test_db: &TestDatabase, query_fragment: &str) {
    timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity
                 WHERE datname = current_database() AND pid <> pg_backend_pid()
                   AND state = 'active' AND wait_event_type = 'Lock'
                   AND position($1 in query) > 0)",
            )
            .bind(query_fragment)
            .fetch_one(&test_db.test_pool)
            .await
            .expect("failed to inspect blocked statement");
            if blocked {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("statement should reach the injected lock");
}

async fn race_cleanup_and_add(cleanup_first: bool) {
    with_test_db(move |test_db| async move {
        let utils = race_utils(&test_db);
        utils.migrate().await.expect("failed to migrate");
        let spec = JobSpec::builder().queue_name("engine").build();
        let first = utils
            .add_job(QueueGcJob { pause: false }, spec.clone())
            .await
            .expect("failed to seed queue");
        utils
            .complete_jobs(&[*first.id()])
            .await
            .expect("failed to empty queue");
        assert_eq!(test_db.get_job_queues().await.len(), 1);
        assert!(test_db.get_jobs().await.is_empty());

        // Pause the first operation after it has found the queue. Advisory
        // locks belong to this isolated database and avoid wall-clock races.
        let (event, table, condition, returned_row) = if cleanup_first {
            ("DELETE", "_private_job_queues", "", "OLD")
        } else {
            ("INSERT", "_private_jobs", "WHEN (NEW.payload::jsonb ->> 'pause' = 'true')", "NEW")
        };
        let sql = formatdoc!(
            r#"
                CREATE FUNCTION graphile_worker.pause_queue_gc_race()
                RETURNS trigger LANGUAGE plpgsql AS $$
                BEGIN
                    PERFORM pg_advisory_xact_lock(548001);
                    RETURN {returned_row};
                END;
                $$;
                CREATE TRIGGER pause_queue_gc_race BEFORE {event}
                    ON graphile_worker.{table}
                    FOR EACH ROW {condition}
                    EXECUTE FUNCTION graphile_worker.pause_queue_gc_race();
            "#
        );
        // All interpolated SQL identifiers and trigger fragments are fixed literals.
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&test_db.test_pool)
            .await
            .expect("failed to inject pause");
        let mut blocker = test_db.test_pool.begin().await.expect("blocker transaction");
        sqlx::query("SELECT pg_advisory_xact_lock(548001)")
            .execute(&mut *blocker)
            .await
            .expect("failed to hold pause lock");

        let (add, cleanup) = if cleanup_first {
            let cleanup_utils = utils.clone();
            let cleanup = tokio::task::spawn_local(async move {
                cleanup_utils.cleanup(&[CleanupTask::GcJobQueues]).await
            });
            wait_for_lock(&test_db, "_private_job_queues").await;
            let add_utils = utils.clone();
            let add = tokio::task::spawn_local(async move {
                add_utils.add_job(QueueGcJob { pause: true }, spec).await
            });
            timeout(Duration::from_secs(5), async {
                while !add.is_finished() {
                    if sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname = current_database()
                         AND state = 'active' AND wait_event_type = 'Lock' AND query LIKE '%add_job%')",
                    ).fetch_one(&test_db.test_pool).await.unwrap() {
                        break;
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            }).await.expect("add should finish or wait for cleanup");
            (add, cleanup)
        } else {
            let add_utils = utils.clone();
            let add = tokio::task::spawn_local(async move {
                add_utils.add_job(QueueGcJob { pause: true }, spec).await
            });
            wait_for_lock(&test_db, "add_job").await;
            let cleanup_utils = utils.clone();
            let cleanup = tokio::task::spawn_local(async move {
                cleanup_utils.cleanup(&[CleanupTask::GcJobQueues]).await
            });
            timeout(Duration::from_secs(5), async {
                while !cleanup.is_finished() {
                    if sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname = current_database()
                         AND state = 'active' AND wait_event_type = 'Lock' AND query LIKE '%_private_job_queues%')",
                    ).fetch_one(&test_db.test_pool).await.unwrap() {
                        break;
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            }).await.expect("cleanup should finish or wait for add");
            (add, cleanup)
        };

        blocker.rollback().await.expect("failed to release pause");
        let added = timeout(Duration::from_secs(5), add)
            .await.expect("add should finish").expect("add task").expect("add job");
        timeout(Duration::from_secs(5), cleanup)
            .await.expect("cleanup should finish").expect("cleanup task").expect("cleanup");
        let jobs = test_db.get_jobs().await;
        let added_row = jobs.iter().find(|j| j.id == *added.id()).expect("added job");
        let executions = Executions::default();
        let worker = Worker::options()
            .database(test_db.database.clone())
            .concurrency(1)
            .listen_os_shutdown_signals(false)
            .add_extension(executions.clone())
            .define_job::<QueueGcJob>()
            .init().await.expect("failed to init worker");
        worker.run_once().await.expect("failed to run worker");
        assert_eq!(executions.0.load(Ordering::SeqCst), 1, "cleanup must not strand the added job: {added_row:?}");
        assert_eq!(added_row.queue_name.as_deref(), Some("engine"), "added job must keep its named queue");
        assert!(test_db.get_jobs().await.is_empty(), "worker should complete the job");
    }).await;
}

#[tokio::test]
async fn queue_cleanup_waits_for_an_inflight_add() {
    race_cleanup_and_add(false).await;
}

#[tokio::test]
async fn add_waits_for_inflight_queue_cleanup() {
    race_cleanup_and_add(true).await;
}

#[tokio::test]
async fn queue_cleanup_does_not_hold_task_cache_while_waiting_for_an_add_transaction() {
    with_test_db(|test_db| async move {
        let utils = race_utils(&test_db);
        utils.migrate().await.expect("failed to migrate");
        let spec = JobSpec::builder().queue_name("engine").build();
        let transaction = test_db.database.begin().await.expect("add transaction");
        let mut scoped = utils.clone().with_executor(&transaction);
        scoped
            .add_job(QueueGcJob { pause: false }, spec.clone())
            .await
            .expect("first add");

        let cleanup_utils = utils.clone();
        let cleanup = tokio::task::spawn_local(async move {
            cleanup_utils
                .cleanup(&[CleanupTask::GcJobQueues, CleanupTask::GcTaskIdentifiers])
                .await
        });
        wait_for_lock(&test_db, "LOCK TABLE").await;

        // The caller must be able to finish its transaction while queue GC
        // waits for its table lock. Typed batch insertion reads the task cache.
        let second_add = timeout(
            Duration::from_secs(2),
            scoped.add_jobs(&[(QueueGcJob { pause: false }, &spec)]),
        )
        .await;
        drop(scoped);
        transaction.commit().await.expect("commit adds");
        timeout(Duration::from_secs(5), cleanup)
            .await
            .expect("cleanup must finish")
            .expect("cleanup task")
            .expect("cleanup");
        second_add
            .expect("queue GC must not block the transaction on the task cache")
            .expect("second add");
    })
    .await;
}

#[tokio::test]
async fn queue_cleanup_bounds_its_wait_for_a_long_running_add_transaction() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");
        let spec = JobSpec::builder().queue_name("engine").build();
        let transaction = test_db.database.begin().await.expect("add transaction");
        utils
            .clone()
            .with_executor(&transaction)
            .add_job(QueueGcJob { pause: false }, spec)
            .await
            .expect("add job");

        let result = timeout(
            Duration::from_secs(5),
            utils.cleanup(&[CleanupTask::GcJobQueues]),
        )
        .await
        .expect("queue GC must not wait indefinitely");
        assert!(result.is_err(), "a held table lock must time out cleanup");
        transaction.commit().await.expect("commit add");
        utils
            .cleanup(&[CleanupTask::GcJobQueues])
            .await
            .expect("cleanup must work after timeout");
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].queue_name.as_deref(), Some("engine"));
    })
    .await;
}
