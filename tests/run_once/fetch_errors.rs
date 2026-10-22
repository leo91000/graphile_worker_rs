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

/// Delegates real PostgreSQL operations, injecting one causally gated fetch fault.
#[derive(Debug)]
struct SiblingFetchFault {
    database: graphile_worker::Database,
    gate: FetchGate,
    claims: std::sync::atomic::AtomicUsize,
    race: Option<bool>,
    claimed: Notify,
    faulted: Notify,
}

impl graphile_worker_database::DbExecutor for SiblingFetchFault {
    /// Leaves all mutation and return SQL on the real driver.
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: graphile_worker_database::DbParams,
    ) -> graphile_worker_database::BoxFuture<'a, Result<u64, graphile_worker_database::DbError>>
    {
        graphile_worker_database::DbExecutor::execute(&self.database, sql, params)
    }

    /// Fails the second claim only after the first handler has started.
    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: graphile_worker_database::DbParams,
    ) -> graphile_worker_database::BoxFuture<
        'a,
        Result<Vec<graphile_worker_database::DbRow>, graphile_worker_database::DbError>,
    > {
        Box::pin(async move {
            if sql.contains("with j as (") {
                let claim = self.claims.fetch_add(1, Ordering::SeqCst);
                if let Some(stream_fault) = self.race {
                    let fault_claim = if stream_fault { 1 } else { 2 };
                    let delayed_claim = if stream_fault { 2 } else { 1 };
                    if claim == fault_claim {
                        self.claimed.notified().await;
                        self.faulted.notify_one();
                        return Err(graphile_worker_database::DbError::with_code(
                            "sibling fetch fault",
                            "40001",
                        ));
                    }
                    if claim == delayed_claim {
                        let result = graphile_worker_database::DbExecutor::fetch_all(
                            &self.database,
                            sql,
                            params,
                        )
                        .await;
                        assert_eq!(
                            result.as_ref().unwrap().len(),
                            1,
                            "fixture must hold a committed claim"
                        );
                        self.claimed.notify_one();
                        self.faulted.notified().await;
                        return result;
                    }
                } else if claim == 1 {
                    self.gate.started.notified().await;
                    // This current-thread fixture cannot poll the healthy task until
                    // this error has propagated through the stream's ready future.
                    self.gate.finish.notify_one();
                    return Err(graphile_worker_database::DbError::with_code(
                        "sibling fetch fault",
                        "40001",
                    ));
                }
            }
            graphile_worker_database::DbExecutor::fetch_all(&self.database, sql, params).await
        })
    }
}

impl graphile_worker_database::DatabaseDriver for SiblingFetchFault {
    /// Supports the Database driver's diagnostic downcast contract.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    /// Delegates initialization and explicit transactions without fault injection.
    fn begin<'a>(
        &'a self,
    ) -> graphile_worker_database::BoxFuture<
        'a,
        Result<graphile_worker_database::DbTransaction, graphile_worker_database::DbError>,
    > {
        Box::pin(self.database.begin())
    }
    /// Preserves the underlying driver's listener behavior.
    fn listen<'a>(
        &'a self,
        channel: &'a str,
    ) -> graphile_worker_database::BoxFuture<
        'a,
        Result<
            Option<graphile_worker_database::NotificationStream>,
            graphile_worker_database::DbError,
        >,
    > {
        Box::pin(self.database.listen(channel))
    }
}

/// Exercises stopped claims with real queue locks and explicit cross-fetch gates.
async fn check_stop_continuations(race: Option<bool>) {
    helpers::with_test_db(move |db| async move {
        let gate = FetchGate::default();
        let worker = db
            .create_worker_options()
            .database(graphile_worker::Database::new(SiblingFetchFault {
                database: db.database.clone(),
                gate: gate.clone(),
                claims: Default::default(),
                race,
                claimed: Notify::new(),
                faulted: Notify::new(),
            }))
            .concurrency(2)
            .add_extension(gate.clone())
            .define_job::<FetchJob>()
            .init()
            .await
            .unwrap();
        let utils = worker.create_utils();
        utils
            .add_job(
                FetchJob {
                    poison: false,
                    wait: race.is_none(),
                },
                JobSpec {
                    queue_name: Some("healthy-backlog".into()),
                    priority: Some(-1),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let pending = utils
            .add_job(
                FetchJob {
                    poison: false,
                    wait: false,
                },
                JobSpec {
                    queue_name: Some(
                        if race.is_some() {
                            "in-flight"
                        } else {
                            "healthy-backlog"
                        }
                        .into(),
                    ),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let error = tokio::time::timeout(std::time::Duration::from_secs(5), worker.run_once())
            .await
            .expect("fetch error must return after draining active work")
            .unwrap_err();
        assert!(error.to_string().contains("sibling fetch fault"));
        assert!(gate.finished.load(Ordering::SeqCst));
        let jobs = db.get_jobs().await;
        assert_eq!(
            jobs.len(),
            1,
            "work not active at the fetch fault must remain pending"
        );
        assert_eq!(jobs[0].id, *pending.id());
        assert_eq!(jobs[0].attempts, 0);
        assert!(jobs[0].locked_by.is_none());
        assert!(db
            .get_job_queues()
            .await
            .iter()
            .all(|queue| queue.locked_by.is_none()));
    })
    .await;
}

/// A stream fetch fault drains the active queued job without consuming its backlog.
#[tokio::test]
async fn stream_fetch_error_stops_named_queue_continuations() {
    check_stop_continuations(None).await;
}

/// A committed continuation claim finishing after a stream fault is returned, not run.
#[tokio::test]
async fn stream_fetch_error_returns_in_flight_continuation() {
    check_stop_continuations(Some(true)).await;
}

/// A continuation fault also stops the stream and returns its in-flight claim.
#[tokio::test]
async fn continuation_fetch_error_returns_in_flight_stream_claim() {
    check_stop_continuations(Some(false)).await;
}
