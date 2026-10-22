#[path = "helpers/mod.rs"]
mod helpers;

use std::num::NonZeroU32;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use graphile_worker::{
    IntoTaskHandlerResult, JobComplete, JobFail, JobSpec, SqlRetryPolicy, TaskHandler,
    WorkerContext, WorkerContextQueueExt,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct RetryJob {
    fail: bool,
    queue: Option<String>,
}

impl TaskHandler for RetryJob {
    const IDENTIFIER: &'static str = "sql_retry_job";

    /// Checks the Rust queue-name helper while executing the real claimed job.
    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        assert_eq!(ctx.get_queue_name().await.unwrap(), self.queue);
        if self.fail {
            Err("retry task failure")
        } else {
            Ok(())
        }
    }
}

/// Faults both claims and outcomes twice, then requires one execution and one hook.
async fn check_transient_outcome(fail: bool, has_queue: bool) {
    helpers::with_test_db(move |db| async move {
        let completed = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicUsize::new(0));
        let worker = db.create_worker_options().define_job::<RetryJob>()
            .concurrency(1)
            .sql_retry_policy(SqlRetryPolicy::new(NonZeroU32::new(3).unwrap()))
            .on(JobComplete, { let count = completed.clone(); move |_| { let count = count.clone(); async move { count.fetch_add(1, Ordering::SeqCst); } } })
            .on(JobFail, { let count = failed.clone(); move |_| { let count = count.clone(); async move { count.fetch_add(1, Ordering::SeqCst); } } })
            .init().await.unwrap();
        let queue = has_queue.then(|| "retry-queue".to_owned());
        let job = worker.create_utils().add_job(RetryJob { fail, queue: queue.clone() }, JobSpec { queue_name: queue, ..Default::default() }).await.unwrap();
        sqlx::raw_sql(include_str!("fixtures/sql_retry_faults.sql")).execute(&db.test_pool).await.unwrap();
        let event = if fail {
            "UPDATE ON graphile_worker._private_jobs FOR EACH ROW WHEN (OLD.locked_by IS NOT NULL AND NEW.locked_by IS NULL)"
        } else { "DELETE ON graphile_worker._private_jobs FOR EACH ROW" };
        helpers::sql::safe_query(format!("CREATE TRIGGER transient_outcome BEFORE {event} EXECUTE FUNCTION graphile_worker.transient_outcome()"))
            .execute(&db.test_pool).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), worker.run_once()).await.expect("bounded retries must finish").unwrap();
        assert_eq!(completed.load(Ordering::SeqCst), usize::from(!fail));
        assert_eq!(failed.load(Ordering::SeqCst), usize::from(fail));
        for sequence in ["claim_retries", "outcome_retries"] {
            let attempts: i64 = helpers::sql::safe_query_scalar(format!("SELECT last_value FROM graphile_worker.{sequence}"))
                .fetch_one(&db.test_pool).await.unwrap();
            assert_eq!(attempts, 3, "each rolled-back statement must be retried twice");
        }
        let jobs = db.get_jobs().await;
        if fail {
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].id, *job.id());
            assert_eq!(jobs[0].attempts, 1, "claim retries must not consume task attempts");
            assert!(jobs[0].locked_by.is_none());
            assert!(jobs[0].last_error.as_ref().unwrap().contains("retry task failure"));
        } else { assert!(jobs.is_empty()); }
        for queue in db.get_job_queues().await { assert!(queue.locked_by.is_none()); assert!(queue.locked_at.is_none()); }
    }).await;
}

/// Unnamed completion retries server-aborted statements without rerunning the task.
#[tokio::test]
async fn unnamed_completion_retries() {
    check_transient_outcome(false, false).await;
}
/// Named completion releases the queue only after persistence succeeds.
#[tokio::test]
async fn named_completion_retries() {
    check_transient_outcome(false, true).await;
}
/// Unnamed failure keeps its consumed attempt and emits one persisted failure hook.
#[tokio::test]
async fn unnamed_failure_retries() {
    check_transient_outcome(true, false).await;
}
/// Named failure retries claim and persistence while preserving queue ownership.
#[tokio::test]
async fn named_failure_retries() {
    check_transient_outcome(true, true).await;
}

/// A caller-owned aborted transaction is returned intact for whole-transaction recovery.
#[tokio::test]
async fn caller_transaction_is_not_retried_by_a_pooled_policy() {
    helpers::with_test_db(|db| async move {
        let worker = db.create_worker_options().sql_retry_policy(SqlRetryPolicy::default()).init().await.unwrap();
        sqlx::raw_sql("CREATE SEQUENCE graphile_worker.transaction_faults; CREATE TABLE graphile_worker.transaction_sentinel(id int);")
            .execute(&db.test_pool).await.unwrap();
        let tx = worker.database().begin().await.unwrap();
        use graphile_worker::DbExecutor;
        tx.execute("INSERT INTO graphile_worker.transaction_sentinel VALUES (1)", Default::default()).await.unwrap();
        let error = tx.execute("DO $$ BEGIN PERFORM nextval('graphile_worker.transaction_faults'); RAISE EXCEPTION USING ERRCODE='40001', MESSAGE='caller transaction'; END $$", Default::default()).await.unwrap_err();
        assert_eq!(error.code(), Some("40001"));
        let error = tx.execute("SELECT 1", Default::default()).await.unwrap_err();
        assert_eq!(error.code(), Some("25P02"), "caller still owns the aborted transaction");
        // PostgreSQL interprets COMMIT of an aborted transaction as ROLLBACK.
        tx.commit().await.unwrap();
        let faults: i64 = sqlx::query_scalar("SELECT last_value FROM graphile_worker.transaction_faults").fetch_one(&db.test_pool).await.unwrap();
        let retained: i64 = sqlx::query_scalar("SELECT count(*) FROM graphile_worker.transaction_sentinel").fetch_one(&db.test_pool).await.unwrap();
        assert_eq!(faults, 1);
        assert_eq!(retained, 0);
    }).await;
}
