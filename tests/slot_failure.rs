use std::sync::atomic::{AtomicBool, Ordering};
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
