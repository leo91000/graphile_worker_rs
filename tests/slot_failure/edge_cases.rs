use super::*;
use graphile_worker::{Database, LocalQueueConfig, RawJobSpec, SweepStaleWorkersOptions};
use graphile_worker_database::{
    BoxFuture, DatabaseDriver, DbError, DbExecutor, DbParams, DbRow, DbTransaction,
    NotificationStream,
};

#[derive(Debug)]
struct ListenFailureDatabase {
    inner: Database,
    wait_for_claim: bool,
}

impl DbExecutor for ListenFailureDatabase {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        self.inner.execute(sql, params)
    }

    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        self.inner.fetch_all(sql, params)
    }
}

impl DatabaseDriver for ListenFailureDatabase {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn begin(&self) -> BoxFuture<'_, Result<DbTransaction, DbError>> {
        Box::pin(self.inner.begin())
    }

    fn listen<'a>(
        &'a self,
        _: &'a str,
    ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async move {
            if self.wait_for_claim {
                // LocalQueue starts prefetching before the notification listener
                // is initialized. Force an actual cached claim before failing it.
                loop {
                    let row = self.inner.fetch_one(
                        "SELECT EXISTS(SELECT 1 FROM graphile_worker._private_jobs WHERE locked_by IS NOT NULL) AS claimed",
                        DbParams::new(),
                    ).await?;
                    if row.try_get::<bool>("claimed")? {
                        break;
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            }
            Err(DbError::new("injected listener failure"))
        })
    }
}

async fn listener_failure_with_batchers(local_queue: bool) {
    with_test_db(move |test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("migrate");
        let queued = utils
            .add_job(PoisonJob { poison: true }, JobSpec::default())
            .await
            .expect("seed job");
        let database = Database::new(ListenFailureDatabase {
            inner: test_db.database.clone(),
            wait_for_claim: local_queue,
        });
        let mut options = Worker::options()
            .database(database)
            .concurrency(2)
            .complete_job_batch_delay(Duration::from_millis(10))
            .fail_job_batch_delay(Duration::from_millis(10))
            .listen_os_shutdown_signals(false)
            .define_job::<PoisonJob>();
        if local_queue {
            options = options.local_queue(LocalQueueConfig::builder().size(10).build());
        }
        let worker = options.init().await.expect("init worker");
        let result = timeout(Duration::from_secs(5), worker.run()).await;
        worker.request_shutdown();
        let result = result.expect("listener error must stop batchers");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("injected listener failure"));
        let jobs = test_db.get_jobs().await;
        let queued = jobs
            .iter()
            .find(|j| j.id == *queued.id())
            .expect("unstarted job");
        assert!(
            queued.locked_by.is_none(),
            "listener error must release cached claims"
        );
        assert_eq!(queued.attempts, 0);
    })
    .await;
}

#[tokio::test]
async fn listener_failure_stops_batchers_in_direct_mode() {
    listener_failure_with_batchers(false).await;
}

#[tokio::test]
async fn listener_failure_releases_cached_jobs_and_stops_batchers() {
    listener_failure_with_batchers(true).await;
}

#[tokio::test]
async fn heartbeat_continues_while_slots_drain_after_a_failure() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("migrate");
        let state = HeldJobState::default();
        let worker = Arc::new(
            Worker::options()
                .database(test_db.database.clone())
                .concurrency(2)
                .poll_interval(Duration::from_millis(20))
                .heartbeat_interval(Duration::from_millis(20))
                .sweep_interval(Duration::from_secs(60))
                .sweep_threshold(Duration::from_millis(200))
                .shutdown_grace_period(Duration::from_secs(1))
                .shutdown_interrupted_job_retry_delay(Duration::ZERO)
                .listen_os_shutdown_signals(false)
                .add_extension(state.clone())
                .define_job::<HeldJob>()
                .define_job::<PoisonJob>()
                .init()
                .await
                .expect("init worker"),
        );
        let runner = worker.clone();
        let run = tokio::task::spawn_local(async move { runner.run().await });
        let held = utils
            .add_job(HeldJob, JobSpec::default())
            .await
            .expect("held job");
        timeout(Duration::from_secs(5), async {
            while !state.started.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("held job must start");
        fail_fetching_poison_jobs(&test_db).await;
        utils
            .add_job(PoisonJob { poison: true }, JobSpec::default())
            .await
            .expect("poison job");
        timeout(Duration::from_secs(5), worker.shutdown_signal().clone())
            .await
            .expect("slot failure must request shutdown");
        sleep(Duration::from_millis(500)).await;
        let sweep = utils
            .sweep_stale_workers(SweepStaleWorkersOptions {
                sweep_threshold: Some(Duration::from_millis(200)),
                recovery_delay: Some(Duration::ZERO),
                dry_run: false,
            })
            .await
            .expect("sweep");
        let jobs = test_db.get_jobs().await;
        let stored = jobs.iter().find(|j| j.id == *held.id()).expect("held job");
        let still_owned = stored.locked_by.as_deref() == Some(worker.worker_id().as_str());
        let result = timeout(Duration::from_secs(5), run)
            .await
            .expect("drain worker")
            .expect("run task");
        assert!(result.is_err());
        assert!(state.ended.load(Ordering::SeqCst));
        assert!(
            sweep.worker_ids.is_empty(),
            "draining worker must stay alive: {:?}",
            sweep.worker_ids
        );
        assert!(
            still_owned,
            "running job must not be recovered during grace period"
        );
    })
    .await;
}

#[tokio::test]
async fn local_queue_slot_release_failure_drains_jobs_and_returns_cached_claims() {
    with_test_db(|test_db| async move {
        let utils = test_db.worker_utils();
        utils.migrate().await.expect("migrate");
        let state = HeldJobState::default();
        let executions = PostFailureExecutions::default();
        let worker = Arc::new(Worker::options().database(test_db.database.clone())
            .concurrency(2).poll_interval(Duration::from_millis(20))
            .local_queue(LocalQueueConfig::builder().size(10).build())
            .shutdown_grace_period(Duration::from_millis(300))
            .shutdown_interrupted_job_retry_delay(Duration::ZERO)
            .listen_os_shutdown_signals(false).add_extension(state.clone())
            .add_extension(executions.clone()).define_job::<HeldJob>()
            .define_job::<PoisonJob>().define_job::<PostFailureJob>()
            .init().await.expect("init worker"));
        let runner = worker.clone();
        let run = tokio::task::spawn_local(async move { runner.run().await });
        let held = utils.add_job(HeldJob, JobSpec::default()).await.expect("held job");
        timeout(Duration::from_secs(5), async {
            while !state.started.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("held job must start");
        sqlx::raw_sql(r#"
            CREATE FUNCTION graphile_worker.fail_poison_completion() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN RAISE EXCEPTION 'injected completion failure'; END; $$;
            CREATE TRIGGER fail_poison_completion BEFORE DELETE ON graphile_worker._private_jobs
                FOR EACH ROW WHEN (OLD.payload::jsonb ->> 'poison' = 'true')
                EXECUTE FUNCTION graphile_worker.fail_poison_completion();
        "#).execute(&test_db.test_pool).await.expect("inject completion failure");
        let added = utils.add_raw_jobs(&[
            RawJobSpec { identifier: PoisonJob::IDENTIFIER.into(), payload: serde_json::json!({"poison":true}), spec: JobSpec::default() },
            RawJobSpec { identifier: PostFailureJob::IDENTIFIER.into(), payload: serde_json::json!({"poison":false}), spec: JobSpec::builder().priority(10).build() },
        ]).await.expect("poison and cached jobs");
        let result = timeout(Duration::from_secs(5), run).await.expect("drain local queue")
            .expect("run task");
        let error = result.expect_err("completion failure must reach Worker::run");
        // Drivers format PostgreSQL's trigger error differently; the public
        // runtime error must still identify the failed release operation.
        assert!(error.to_string().contains("releasing a job"), "{error:?}");
        assert!(state.ended.load(Ordering::SeqCst));
        assert_eq!(executions.0.load(Ordering::SeqCst), 0);
        let jobs = test_db.get_jobs().await;
        for id in [*held.id(), *added[1].id()] {
            let stored = jobs.iter().find(|j| j.id == id).expect("returned job");
            assert!(stored.locked_by.is_none(), "held and cached claims must be returned");
            assert_eq!(stored.attempts, 0);
        }
    }).await;
}
