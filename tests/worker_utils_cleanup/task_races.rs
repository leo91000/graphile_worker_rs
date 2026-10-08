use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use graphile_worker::errors::GraphileWorkerError;
use graphile_worker::{
    worker_utils::types::CleanupTask, IntoTaskHandlerResult, JobSpec, TaskHandler, Worker,
    WorkerContext, WorkerUtils,
};
use indoc::formatdoc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::helpers::{with_test_db, TestDatabase};

// queue_races.rs keeps its load of the fixture private, so this module loads
// the file again.
#[allow(clippy::duplicate_mod)]
#[path = "race_database.rs"]
mod race_database;

const TASK_GC_JOB: &str = "task_gc_job";
const OTHER_TASK: &str = "other_task";
const ARRIVING_TASK: &str = "arriving_task";
const PAUSE_KEY: i64 = 548_002;
const LOCK_TIMEOUT_SQLSTATE: &str = "55P03";

type CleanupOutcome = (Result<(), GraphileWorkerError>, Duration);

#[derive(Clone, Debug, Default)]
struct Executions(Arc<AtomicU32>);

#[derive(Clone, Deserialize, Serialize)]
struct TaskGcJob {
    #[serde(default)]
    pause: bool,
}

impl TaskHandler for TaskGcJob {
    const IDENTIFIER: &'static str = TASK_GC_JOB;

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        ctx.get_ext::<Executions>()
            .expect("execution counter")
            .0
            .fetch_add(1, Ordering::SeqCst);
        Ok::<(), String>(())
    }
}

fn race_utils(test_db: &TestDatabase) -> WorkerUtils {
    WorkerUtils::new(
        graphile_worker_database::Database::new(race_database::RaceDatabase(
            test_db.database.clone(),
        )),
        "graphile_worker",
    )
}

fn spawn_task_cleanup(utils: &WorkerUtils) -> JoinHandle<CleanupOutcome> {
    let utils = utils.clone();
    tokio::task::spawn_local(async move {
        let started = Instant::now();
        let result = utils.cleanup(&[CleanupTask::GcTaskIdentifiers]).await;
        (result, started.elapsed())
    })
}

fn is_lock_timeout(error: &GraphileWorkerError) -> bool {
    matches!(
        error,
        GraphileWorkerError::SqlError(error) if error.code() == Some(LOCK_TIMEOUT_SQLSTATE)
    )
}

/// Leaves an unused `task_gc_job` row behind: its only job is completed.
async fn seed_unused_task(utils: &WorkerUtils) {
    let job = utils
        .add_raw_job(TASK_GC_JOB, json!({}), JobSpec::default())
        .await
        .expect("failed to seed task");
    utils
        .complete_jobs(&[*job.id()])
        .await
        .expect("failed to empty task");
}

async fn task_id(test_db: &TestDatabase, identifier: &str) -> Option<i32> {
    sqlx::query_scalar("SELECT id FROM graphile_worker._private_tasks WHERE identifier = $1")
        .bind(identifier)
        .fetch_optional(&test_db.test_pool)
        .await
        .expect("failed to read task id")
}

/// Returns the job's `task_id` and whether a task row with that id exists.
async fn task_reference(test_db: &TestDatabase, job_id: i64) -> (i32, bool) {
    sqlx::query_as(
        "SELECT jobs.task_id,
                EXISTS(SELECT 1 FROM graphile_worker._private_tasks tasks
                       WHERE tasks.id = jobs.task_id)
         FROM graphile_worker._private_jobs jobs WHERE jobs.id = $1",
    )
    .bind(job_id)
    .fetch_one(&test_db.test_pool)
    .await
    .expect("failed to read the job's task reference")
}

/// A transaction that holds `ROW EXCLUSIVE` on `_private_tasks` until it ends.
async fn open_task_writer(test_db: &TestDatabase) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut writer = test_db.test_pool.begin().await.expect("writer transaction");
    sqlx::query("SELECT graphile_worker.add_job($1, '{}'::json)")
        .bind(OTHER_TASK)
        .execute(&mut *writer)
        .await
        .expect("failed to write the task table");
    writer
}

/// Polls until the operation waits for an ungranted lock of `mode` on
/// `_private_tasks` (true) or finishes without having been seen waiting (false).
async fn wait_for_ungranted_lock<T>(
    test_db: &TestDatabase,
    mode: &str,
    operation: &JoinHandle<T>,
) -> bool {
    timeout(Duration::from_secs(3), async {
        loop {
            if operation.is_finished() {
                return false;
            }
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_locks
                 WHERE locktype = 'relation' AND NOT granted AND mode = $1
                   AND database = (SELECT oid FROM pg_database WHERE datname = current_database())
                   AND relation = 'graphile_worker._private_tasks'::regclass)",
            )
            .bind(mode)
            .fetch_one(&test_db.test_pool)
            .await
            .expect("failed to inspect locks");
            if waiting {
                return true;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("operation should finish or wait for a lock")
}

async fn wait_for_paused_add(test_db: &TestDatabase) {
    timeout(Duration::from_secs(3), async {
        loop {
            let paused: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity
                 WHERE datname = current_database() AND pid <> pg_backend_pid()
                   AND wait_event_type = 'Lock' AND wait_event = 'advisory'
                   AND position('add_job' in query) > 0)",
            )
            .fetch_one(&test_db.test_pool)
            .await
            .expect("failed to inspect blocked statement");
            if paused {
                return;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("add should reach the injected pause");
}

/// Pauses every insert of a job whose payload asks for it, after the add has
/// already written the task table, until the advisory lock is released.
async fn install_add_pause(test_db: &TestDatabase) {
    let sql = formatdoc!(
        r#"
            CREATE FUNCTION graphile_worker.pause_task_gc_race()
            RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN
                PERFORM pg_advisory_xact_lock({PAUSE_KEY});
                RETURN NEW;
            END;
            $$;
            CREATE TRIGGER pause_task_gc_race BEFORE INSERT
                ON graphile_worker._private_jobs
                FOR EACH ROW WHEN (NEW.payload::jsonb ->> 'pause' = 'true')
                EXECUTE FUNCTION graphile_worker.pause_task_gc_race();
        "#
    );
    // The key is a fixed literal.
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
        .execute(&test_db.test_pool)
        .await
        .expect("failed to inject pause");
}

async fn hold_add_pause(test_db: &TestDatabase) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut blocker = test_db
        .test_pool
        .begin()
        .await
        .expect("blocker transaction");
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(PAUSE_KEY)
        .execute(&mut *blocker)
        .await
        .expect("failed to hold pause lock");
    blocker
}

/// Defines a worker for `task_gc_job` once the race is over, so its `init`
/// resolves the task row as it then stands, and runs the jobs that are due.
async fn run_race_task_once(test_db: &TestDatabase) -> u32 {
    let executions = Executions::default();
    let worker = Worker::options()
        .database(test_db.database.clone())
        .concurrency(1)
        .listen_os_shutdown_signals(false)
        .add_extension(executions.clone())
        .define_job::<TaskGcJob>()
        .init()
        .await
        .expect("failed to init worker");
    worker.run_once().await.expect("failed to run worker");
    executions.0.load(Ordering::SeqCst)
}

#[tokio::test]
async fn keeps_task_of_job_added_during_cleanup() {
    with_test_db(|test_db| async move {
        let utils = race_utils(&test_db);
        utils.migrate().await.expect("failed to migrate");
        seed_unused_task(&utils).await;
        let original_id = task_id(&test_db, TASK_GC_JOB).await.expect("seeded task");
        install_add_pause(&test_db).await;
        let blocker = hold_add_pause(&test_db).await;

        let add_utils = utils.clone();
        let add = tokio::task::spawn_local(async move {
            add_utils
                .add_raw_job(TASK_GC_JOB, json!({ "pause": true }), JobSpec::default())
                .await
        });
        wait_for_paused_add(&test_db).await;
        let cleanup = spawn_task_cleanup(&utils);
        let cleanup_waited =
            wait_for_ungranted_lock(&test_db, "ShareRowExclusiveLock", &cleanup).await;

        blocker.rollback().await.expect("failed to release pause");
        let added = timeout(Duration::from_secs(5), add)
            .await
            .expect("add should finish")
            .expect("add task")
            .expect("add job");
        let (cleanup_result, _) = timeout(Duration::from_secs(5), cleanup)
            .await
            .expect("cleanup should finish")
            .expect("cleanup task");

        assert!(
            cleanup_waited,
            "task cleanup must wait for an add that is past its task insert"
        );
        cleanup_result.expect("cleanup");
        assert_eq!(
            task_id(&test_db, TASK_GC_JOB).await,
            Some(original_id),
            "cleanup must keep the task of the added job"
        );
        assert_eq!(
            task_reference(&test_db, *added.id()).await,
            (original_id, true),
            "the added job must reference the original task row"
        );

        assert_eq!(
            run_race_task_once(&test_db).await,
            1,
            "the added job must run"
        );
        assert!(test_db.get_jobs().await.is_empty());
    })
    .await;
}

#[tokio::test]
async fn recreates_task_for_job_added_while_cleanup_waits() {
    with_test_db(|test_db| async move {
        let utils = race_utils(&test_db);
        utils.migrate().await.expect("failed to migrate");
        seed_unused_task(&utils).await;

        let mut exercised = false;
        for round in 1..=5 {
            let original_id = task_id(&test_db, TASK_GC_JOB).await.expect("task row");
            let writer = open_task_writer(&test_db).await;
            let cleanup = spawn_task_cleanup(&utils);
            let cleanup_waited =
                wait_for_ungranted_lock(&test_db, "ShareRowExclusiveLock", &cleanup).await;
            let add_utils = utils.clone();
            let add = tokio::task::spawn_local(async move {
                add_utils
                    .add_raw_job(TASK_GC_JOB, json!({}), JobSpec::default())
                    .await
            });
            let add_waited = wait_for_ungranted_lock(&test_db, "RowExclusiveLock", &add).await;

            writer.commit().await.expect("failed to commit writer");
            let added = timeout(Duration::from_secs(5), add)
                .await
                .expect("add should finish")
                .expect("add task")
                .expect("add job");
            let (cleanup_result, _) = timeout(Duration::from_secs(5), cleanup)
                .await
                .expect("cleanup should finish")
                .expect("cleanup task");
            let cleanup_returned_ok = match cleanup_result {
                Ok(()) => true,
                Err(error) if is_lock_timeout(&error) => false,
                Err(error) => panic!("round {round}: cleanup failed: {error}"),
            };

            let (task_id_of_job, task_exists) = task_reference(&test_db, *added.id()).await;
            assert!(
                task_exists,
                "round {round}: the added job references task {task_id_of_job}, which does not exist"
            );
            assert_eq!(
                run_race_task_once(&test_db).await,
                1,
                "round {round}: the added job must run"
            );

            let recreated_id = task_id(&test_db, TASK_GC_JOB).await.expect("task row");
            if cleanup_waited && add_waited && cleanup_returned_ok && recreated_id != original_id {
                exercised = true;
                break;
            }
        }
        assert!(
            exercised,
            "no round had task cleanup delete the task row while an add waited behind it"
        );
    })
    .await;
}

#[tokio::test]
async fn task_cleanup_stops_waiting_after_lock_timeout_and_deletes_nothing() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        seed_unused_task(&utils).await;
        let writer = open_task_writer(&test_db).await;
        let cleanup = spawn_task_cleanup(&utils);
        assert!(
            wait_for_ungranted_lock(&test_db, "ShareRowExclusiveLock", &cleanup).await,
            "task cleanup must wait for the open writer"
        );
        sleep(Duration::from_millis(300)).await;
        writer.commit().await.expect("failed to commit writer");
        let (result, elapsed) = timeout(Duration::from_secs(5), cleanup)
            .await
            .expect("cleanup should finish")
            .expect("cleanup task");
        result.expect("cleanup after the writer committed");
        assert!(
            (Duration::from_millis(300)..=Duration::from_secs(2)).contains(&elapsed),
            "cleanup should return once the writer commits, took {elapsed:?}"
        );
        let tasks = test_db.get_tasks().await;
        assert!(!tasks.iter().any(|task| task == TASK_GC_JOB), "{tasks:?}");
        assert!(tasks.iter().any(|task| task == OTHER_TASK), "{tasks:?}");

        seed_unused_task(&utils).await;
        let writer = open_task_writer(&test_db).await;
        let cleanup = spawn_task_cleanup(&utils);
        assert!(
            wait_for_ungranted_lock(&test_db, "ShareRowExclusiveLock", &cleanup).await,
            "task cleanup must wait for the open writer"
        );
        let arriving = timeout(
            Duration::from_secs(2),
            utils.add_raw_job(ARRIVING_TASK, json!({}), JobSpec::default()),
        )
        .await
        .expect("an add must not wait behind cleanup past its lock timeout")
        .expect("arriving add");
        let (result, elapsed) = timeout(Duration::from_secs(5), cleanup)
            .await
            .expect("cleanup should finish")
            .expect("cleanup task");
        let error = result.expect_err("a writer held open must time cleanup out");
        assert!(is_lock_timeout(&error), "unexpected error: {error}");
        assert!(
            (Duration::from_secs(1)..=Duration::from_secs(3)).contains(&elapsed),
            "cleanup should give up after one lock timeout, took {elapsed:?}"
        );
        assert!(
            task_id(&test_db, TASK_GC_JOB).await.is_some(),
            "a timed-out cleanup must delete nothing"
        );

        writer.commit().await.expect("failed to commit writer");
        utils
            .cleanup(&[CleanupTask::GcTaskIdentifiers])
            .await
            .expect("cleanup must work after timeout");
        let tasks = test_db.get_tasks().await;
        assert!(!tasks.iter().any(|task| task == TASK_GC_JOB), "{tasks:?}");
        assert!(tasks.iter().any(|task| task == OTHER_TASK), "{tasks:?}");
        assert!(tasks.iter().any(|task| task == ARRIVING_TASK), "{tasks:?}");
        assert!(
            task_reference(&test_db, *arriving.id()).await.1,
            "the add that waited behind cleanup must keep its task"
        );
    })
    .await;
}

#[tokio::test]
async fn task_cleanup_does_not_hold_task_cache_while_waiting_for_an_add_transaction() {
    with_test_db(|test_db| async move {
        let utils = race_utils(&test_db);
        utils.migrate().await.expect("failed to migrate");
        let spec = JobSpec::default();
        let transaction = test_db.database.begin().await.expect("add transaction");
        let mut scoped = utils.clone().with_executor(&transaction);
        scoped
            .add_job(TaskGcJob { pause: false }, spec.clone())
            .await
            .expect("first add");

        let cleanup = spawn_task_cleanup(&utils);
        let cleanup_waited =
            wait_for_ungranted_lock(&test_db, "ShareRowExclusiveLock", &cleanup).await;
        assert!(
            cleanup_waited,
            "task cleanup must wait for the open add transaction"
        );

        // Typed batch insertion reads the task cache.
        let second_add = timeout(
            Duration::from_millis(500),
            scoped.add_jobs(&[(TaskGcJob { pause: false }, &spec)]),
        )
        .await;
        let cleanup_pending = !cleanup.is_finished();
        drop(scoped);
        transaction.commit().await.expect("commit adds");
        let (cleanup_result, _) = timeout(Duration::from_secs(5), cleanup)
            .await
            .expect("cleanup must finish")
            .expect("cleanup task");

        second_add
            .expect("task cleanup must not block the transaction on the task cache")
            .expect("second add");
        assert!(
            cleanup_pending,
            "cleanup must still be waiting when the second add completes"
        );
        cleanup_result.expect("cleanup");
        let jobs = test_db.get_jobs().await;
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|job| job.task_identifier == TASK_GC_JOB));
    })
    .await;
}
