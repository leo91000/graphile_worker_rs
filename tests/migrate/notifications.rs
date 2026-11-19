use super::*;

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, LocalQueueConfig, TaskHandler, Worker, WorkerContext,
};
use serde::{Deserialize, Serialize};
use tokio::{sync::Notify, time::timeout};

const DEADLINE: Duration = Duration::from_secs(7);

#[derive(Clone, Debug, Default)]
struct RunningJob {
    started: Arc<Notify>,
    finish: Arc<Notify>,
}

#[derive(Deserialize, Serialize)]
struct InFlight;

impl TaskHandler for InFlight {
    const IDENTIFIER: &'static str = "migration_in_flight";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        let job = ctx.get_ext::<RunningJob>().unwrap();
        job.started.notify_one();
        job.finish.notified().await;
    }
}

#[tokio::test]
async fn committed_breaking_migration_drains_direct_and_local_workers() {
    for use_local_queue in [false, true] {
        with_test_db(move |db| async move {
            let state = RunningJob::default();
            let mut options = db
                .create_worker_options()
                .define_job::<InFlight>()
                .add_extension(state.clone())
                .concurrency(1)
                .poll_interval(Duration::from_secs(60))
                .listen_os_shutdown_signals(false);
            if use_local_queue {
                options = options.local_queue(LocalQueueConfig::default().with_size(10));
            }
            let worker: Arc<Worker> = Arc::new(options.init().await.unwrap());
            let first = worker.create_utils().add_job(InFlight, JobSpec::default()).await.unwrap();
            let running = tokio::task::spawn_local({
                let worker = worker.clone();
                async move { worker.run().await }
            });
            timeout(DEADLINE, state.started.notified())
                .await
                .expect("worker never started processing");

            // PostgreSQL notifications are shared by all schemas in a database.
            // Announce a different schema's breaking migration on the same channel.
            query("create schema unrelated")
                .execute(&db.test_pool).await.unwrap();
            query("create table unrelated.migrations (id int, breaking boolean)")
                .execute(&db.test_pool).await.unwrap();
            query("insert into unrelated.migrations values (999999, true)")
                .execute(&db.test_pool).await.unwrap();
            query(r#"select pg_notify('worker:migrate', '{"migrationNumber":999999,"breaking":true}')"#)
                .execute(&db.test_pool).await.unwrap();

            let sql = "insert into graphile_worker.migrations (id, breaking) values (999999, true)";
            let notify = r#"select pg_notify('worker:migrate', '{"migrationNumber":999999,"breaking":true}')"#;
            let mut tx = db.test_pool.begin().await.unwrap();
            query(sql).execute(&mut *tx).await.unwrap();
            query(notify).execute(&mut *tx).await.unwrap();
            tx.rollback().await.unwrap();

            let mut tx = db.test_pool.begin().await.unwrap();
            query("insert into graphile_worker.migrations (id, breaking) values (999998, false)")
                .execute(&mut *tx).await.unwrap();
            query(r#"select pg_notify('worker:migrate', '{"migrationNumber":999998,"breaking":false}')"#)
                .execute(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();

            let second = worker.create_utils().add_job(InFlight, JobSpec {
                run_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
                ..Default::default()
            }).await.unwrap();
            let shutdown = worker.shutdown_signal().clone();
            assert!(timeout(Duration::from_millis(150), shutdown.clone()).await.is_err());

            let mut tx = db.test_pool.begin().await.unwrap();
            query(sql).execute(&mut *tx).await.unwrap();
            query(notify).execute(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();

            timeout(DEADLINE, shutdown).await.expect("committed migration did not stop the worker");
            assert!(!running.is_finished(), "the running handler must be drained");
            state.finish.notify_one();
            let error = timeout(DEADLINE, running).await.unwrap().unwrap().unwrap_err();
            assert!(error.to_string().contains("999999"), "{error}");
            let jobs = db.get_jobs().await;
            assert!(jobs.iter().all(|job| job.id != *first.id()));
            let second = jobs.iter().find(|job| job.id == *second.id()).unwrap();
            assert_eq!(second.attempts, 0);
            assert!(second.locked_by.is_none());
        }).await;
    }
}

#[tokio::test]
async fn migrator_announces_only_committed_revisions_with_breaking_metadata() {
    with_test_db(|db| async move {
        let mut listener = db.database.listen("worker:migrate").await.unwrap().unwrap();
        migrate(&db.database, "graphile_worker").await.unwrap();

        for migration in GRAPHILE_WORKER_MIGRATIONS {
            let notice = timeout(DEADLINE, listener.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let payload: serde_json::Value = serde_json::from_str(&notice.payload).unwrap();
            assert_eq!(notice.channel, "worker:migrate");
            assert_eq!(payload["migrationNumber"], migration.migration_number());
            assert_eq!(payload["breaking"], migration.is_breaking());
        }
        migrate(&db.database, "graphile_worker").await.unwrap();
        assert!(timeout(Duration::from_millis(150), listener.next())
            .await
            .is_err());
    })
    .await;
}

#[tokio::test]
async fn first_poll_rechecks_schema_when_notifications_are_disabled() {
    with_test_db(|db| async move {
        let worker = db
            .create_worker_options()
            .use_notification_delivery(false)
            .listen_os_shutdown_signals(false)
            .init()
            .await
            .unwrap();
        query("insert into graphile_worker.migrations (id, breaking) values (999999, true)")
            .execute(&db.test_pool)
            .await
            .unwrap();

        let result = timeout(DEADLINE, worker.run())
            .await
            .expect("poll missed revision");
        assert!(result.unwrap_err().to_string().contains("999999"));
    })
    .await;
}
