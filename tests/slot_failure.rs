use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use graphile_worker::{IntoTaskHandlerResult, JobSpec, TaskHandler, Worker, WorkerContext};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, timeout, Duration};

use crate::helpers::{with_test_db, TestDatabase};

mod helpers;

#[derive(Clone, Debug, Default)]
struct HeldJobState {
    started: Arc<AtomicBool>,
    ended: Arc<AtomicBool>,
}

struct EndGuard(Arc<AtomicBool>);

impl Drop for EndGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Deserialize, Serialize)]
struct HeldJob;

impl TaskHandler for HeldJob {
    const IDENTIFIER: &'static str = "slot_failure_held_job";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let state = ctx.get_ext::<HeldJobState>().expect("state extension");
        let _guard = EndGuard(state.ended.clone());
        state.started.store(true, Ordering::SeqCst);
        std::future::pending::<()>().await;
        Ok::<(), String>(())
    }
}

#[derive(Deserialize, Serialize)]
struct PoisonJob {
    poison: bool,
}

#[derive(Clone, Debug, Default)]
struct PostFailureExecutions(Arc<AtomicU32>);

#[derive(Deserialize, Serialize)]
struct PostFailureJob {
    poison: bool,
}

impl TaskHandler for PostFailureJob {
    const IDENTIFIER: &'static str = "slot_failure_post_failure_job";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        ctx.get_ext::<PostFailureExecutions>()
            .expect("post-failure counter")
            .0
            .fetch_add(1, Ordering::SeqCst);
        Ok::<(), String>(())
    }
}

impl TaskHandler for PoisonJob {
    const IDENTIFIER: &'static str = "slot_failure_poison_job";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Ok::<(), String>(())
    }
}

async fn fail_fetching_poison_jobs(test_db: &TestDatabase) {
    sqlx::raw_sql(
        r#"
        CREATE FUNCTION graphile_worker.fail_poison_fetch()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            RAISE EXCEPTION 'injected fetch failure';
        END;
        $$;
        CREATE TRIGGER fail_poison_fetch BEFORE UPDATE
            ON graphile_worker._private_jobs
            FOR EACH ROW
            WHEN (OLD.payload::jsonb ? 'poison')
            EXECUTE FUNCTION graphile_worker.fail_poison_fetch();
        "#,
    )
    .execute(&test_db.test_pool)
    .await
    .expect("failed to inject fetch failure");
}

#[tokio::test]
async fn run_waits_for_running_jobs_when_another_slot_fails() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");
        let state = HeldJobState::default();

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_millis(50))
                .shutdown_grace_period(Duration::from_millis(300))
                .shutdown_interrupted_job_retry_delay(Duration::ZERO)
                .listen_os_shutdown_signals(false)
                .add_extension(state.clone())
                .define_job::<HeldJob>()
                .define_job::<PoisonJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );
        let worker_for_run = Arc::clone(&worker);
        let handle = tokio::task::spawn_local(async move { worker_for_run.run().await });

        let held = utils
            .add_job(HeldJob, JobSpec::default())
            .await
            .expect("failed to add held job");
        timeout(Duration::from_secs(5), async {
            while !state.started.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("held job should start");

        fail_fetching_poison_jobs(&test_db).await;
        utils
            .add_job(PoisonJob { poison: true }, JobSpec::default())
            .await
            .expect("failed to add poison job");

        let result = timeout(Duration::from_secs(5), handle)
            .await
            .expect("worker should stop after a slot fails")
            .expect("worker task should not panic");
        let held_job_ended_before_run_returned = state.ended.load(Ordering::SeqCst);

        assert!(result.is_err(), "run should report the slot's error");
        assert!(
            held_job_ended_before_run_returned,
            "run must not return while another slot is still running a job"
        );
        let jobs = test_db.get_jobs().await;
        let held = jobs
            .iter()
            .find(|j| j.id == *held.id())
            .expect("held job should still exist");
        assert!(held.locked_by.is_none(), "held job should be released");
        assert_eq!(held.attempts, 0, "held job should not spend an attempt");
    })
    .await;
}

#[tokio::test]
async fn run_returns_after_a_slot_fails_with_batched_completions() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");

        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_millis(50))
                .complete_job_batch_delay(Duration::from_millis(10))
                .fail_job_batch_delay(Duration::from_millis(10))
                .listen_os_shutdown_signals(false)
                .define_job::<PoisonJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );
        let worker_for_run = Arc::clone(&worker);
        let handle = tokio::task::spawn_local(async move { worker_for_run.run().await });

        fail_fetching_poison_jobs(&test_db).await;
        utils
            .add_job(PoisonJob { poison: true }, JobSpec::default())
            .await
            .expect("failed to add poison job");

        let result = timeout(Duration::from_secs(5), handle)
            .await
            .expect("worker with batchers should stop after a slot fails")
            .expect("worker task should not panic");

        assert!(result.is_err(), "run should report the slot's error");
    })
    .await;
}

#[tokio::test]
async fn slot_failure_does_not_start_jobs_from_buffered_signals() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("failed to migrate");
        let state = HeldJobState::default();
        let executions = PostFailureExecutions::default();
        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_millis(20))
                .shutdown_grace_period(Duration::from_millis(200))
                .shutdown_interrupted_job_retry_delay(Duration::ZERO)
                .listen_os_shutdown_signals(false)
                .add_extension(state.clone())
                .add_extension(executions.clone())
                .define_job::<HeldJob>()
                .define_job::<PostFailureJob>()
                .init()
                .await
                .expect("failed to init worker"),
        );
        let worker_for_run = Arc::clone(&worker);
        let handle = tokio::task::spawn_local(async move { worker_for_run.run().await });
        utils
            .add_job(HeldJob, JobSpec::default())
            .await
            .expect("held job");
        timeout(Duration::from_secs(5), async {
            while !state.started.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("held job should start");

        // The first fetch fails, but subsequent fetches would succeed. This
        // exposes slots consuming signals left buffered after channel closure.
        sqlx::raw_sql(
            r#"
            CREATE SEQUENCE graphile_worker.poison_fetch_calls;
            CREATE FUNCTION graphile_worker.fail_first_poison_fetch()
            RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN
                IF nextval('graphile_worker.poison_fetch_calls') = 1 THEN
                    RAISE EXCEPTION 'injected fetch failure';
                END IF;
                RETURN NEW;
            END;
            $$;
            CREATE TRIGGER fail_first_poison_fetch BEFORE UPDATE
                ON graphile_worker._private_jobs FOR EACH ROW
                WHEN (OLD.payload::jsonb ? 'poison')
                EXECUTE FUNCTION graphile_worker.fail_first_poison_fetch();
            "#,
        )
        .execute(&test_db.test_pool)
        .await
        .expect("failed to inject fetch failure");
        let queued = utils
            .add_job(PostFailureJob { poison: true }, JobSpec::default())
            .await
            .expect("post-failure job");
        let result = timeout(Duration::from_secs(5), handle)
            .await
            .expect("worker should drain slots")
            .expect("worker task should not panic");
        assert!(result.is_err(), "slot failure should be returned");
        assert!(
            state.ended.load(Ordering::SeqCst),
            "held job must stop before run returns"
        );
        assert_eq!(
            executions.0.load(Ordering::SeqCst),
            0,
            "buffered signals must not start another job after a slot fails"
        );
        let jobs = test_db.get_jobs().await;
        let queued = jobs
            .iter()
            .find(|j| j.id == *queued.id())
            .expect("unstarted job");
        assert_eq!(queued.attempts, 0);
        assert!(queued.locked_by.is_none());
    })
    .await;
}
