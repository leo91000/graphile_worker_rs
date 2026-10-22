use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use graphile_worker::{errors::GraphileWorkerError, runner::WorkerRuntimeError};
use tokio::sync::Notify;

use super::*;

#[derive(Clone, Debug, Default)]
struct FetchGate {
    started: Arc<Notify>,
    finish: Arc<Notify>,
    finished: Arc<AtomicBool>,
}

#[derive(Serialize, Deserialize)]
struct FetchJob {
    poison: bool,
    wait: bool,
}

impl TaskHandler for FetchJob {
    const IDENTIFIER: &'static str = "run_once_fetch_error";

    /// Holds healthy work until the test has observed the failed sibling fetch.
    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        assert!(
            !self.poison,
            "a rejected claim must never reach its handler"
        );
        let gate = ctx.get_ext::<FetchGate>().unwrap();
        gate.started.notify_one();
        if self.wait {
            gate.finish.notified().await;
        }
        gate.finished.store(true, Ordering::SeqCst);
        Ok::<(), String>(())
    }
}

/// Rejects claims using a real server error and observes rollback via a sequence.
async fn check_fetch_error(with_sibling: bool, named_followup: bool) {
    helpers::with_test_db(move |db| async move {
        let gate = FetchGate::default();
        let worker = Rc::new(
            db.create_worker_options()
                .concurrency(if named_followup { 1 } else { 2 })
                .add_extension(gate.clone())
                .define_job::<FetchJob>()
                .init()
                .await
                .unwrap(),
        );
        let utils = worker.create_utils();
        if with_sibling {
            utils
                .add_job(
                    FetchJob {
                        poison: false,
                        wait: !named_followup,
                    },
                    JobSpec {
                        priority: Some(-1),
                        queue_name: named_followup.then(|| "fetch-queue".into()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
        let poisoned = utils
            .add_job(
                FetchJob {
                    poison: true,
                    wait: false,
                },
                JobSpec {
                    queue_name: named_followup.then(|| "fetch-queue".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE SEQUENCE graphile_worker.fetch_attempts MINVALUE 0 START 0; \
             CREATE FUNCTION graphile_worker.reject_claim() RETURNS trigger LANGUAGE plpgsql AS $$ \
             BEGIN PERFORM nextval('graphile_worker.fetch_attempts'); \
               RAISE EXCEPTION USING ERRCODE='40001', MESSAGE='injected fetch failure'; END $$; \
             CREATE TRIGGER reject_claim BEFORE UPDATE ON graphile_worker._private_jobs \
             FOR EACH ROW WHEN (OLD.locked_by IS NULL AND NEW.locked_by IS NOT NULL \
               AND OLD.payload::jsonb->>'poison' = 'true') \
             EXECUTE FUNCTION graphile_worker.reject_claim();",
        )
        .execute(&db.test_pool)
        .await
        .unwrap();
        let running = worker.clone();
        let run = spawn_local(async move { running.run_once().await });
        if with_sibling && !named_followup {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                gate.started.notified().await;
                loop {
                    let called: bool =
                        sqlx::query_scalar("SELECT is_called FROM graphile_worker.fetch_attempts")
                            .fetch_one(&db.test_pool)
                            .await
                            .unwrap();
                    if called {
                        break;
                    }
                    sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("fetch fault must occur while healthy work is active");
            assert!(
                !run.is_finished(),
                "a fetch error must not cancel the healthy handler"
            );
            gate.finish.notify_one();
        }
        let error = tokio::time::timeout(std::time::Duration::from_secs(5), run)
            .await
            .expect("run_once must finish")
            .unwrap()
            .expect_err("fetch failures must not report an empty successful run");
        assert!(matches!(error, WorkerRuntimeError::ProcessJob(_)));
        let mut source: &dyn std::error::Error = &error;
        loop {
            if let Some(GraphileWorkerError::SqlError(error)) =
                source.downcast_ref::<GraphileWorkerError>()
            {
                assert_eq!(error.code(), Some("40001"));
                assert!(error.to_string().contains("injected fetch failure"));
                break;
            }
            source = source
                .source()
                .expect("fetch diagnostic must remain in the error chain");
        }
        assert_eq!(gate.finished.load(Ordering::SeqCst), with_sibling);
        let jobs = db.get_jobs().await;
        assert_eq!(
            jobs.len(),
            1,
            "healthy work must be persisted before return"
        );
        assert_eq!(jobs[0].id, *poisoned.id());
        assert_eq!(
            jobs[0].attempts, 0,
            "rejected claims must roll back their attempt"
        );
        assert!(jobs[0].locked_by.is_none());
        assert!(jobs[0].locked_at.is_none());
        for queue in db.get_job_queues().await {
            assert!(queue.locked_by.is_none());
        }
    })
    .await;
}

/// Initial claim failures preserve the SQLSTATE and leave the job available.
#[tokio::test]
async fn initial_fetch_error_propagates() {
    check_fetch_error(false, false).await;
}

/// A later stream failure drains an already active healthy sibling.
#[tokio::test]
async fn stream_fetch_error_drains_sibling() {
    check_fetch_error(true, false).await;
}

/// The named-queue continuation also returns claim errors after persisting work.
#[tokio::test]
async fn named_followup_fetch_error_propagates() {
    check_fetch_error(true, true).await;
}
