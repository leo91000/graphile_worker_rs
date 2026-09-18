//! Fault-injection tests for retaining claims across failed or cancelled returns.
use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker_database::{
    BoxFuture, DatabaseDriver, DbError, DbExecutor, DbParams, DbRow, DbTransaction,
    NotificationStream,
};
use graphile_worker_job::Job;

use super::*;

#[derive(Debug, Default)]
struct ReturnDriver {
    fail: AtomicBool,
    block: AtomicBool,
    calls: AtomicUsize,
}

#[derive(Debug)]
struct TestDriver(Arc<ReturnDriver>);

impl DbExecutor for TestDriver {
    /// Injects failures or a pending response only for the job-return SQL.
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        _params: DbParams,
    ) -> BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move {
            assert!(sql.contains("_private_return_jobs"));
            self.0.calls.fetch_add(1, Ordering::SeqCst);
            if self.0.block.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            if self.0.fail.load(Ordering::SeqCst) {
                return Err(DbError::new("injected return failure"));
            }
            Ok(1)
        })
    }

    /// Fails if a return unexpectedly starts fetching jobs.
    fn fetch_all<'a>(
        &'a self,
        _sql: &'a str,
        _params: DbParams,
    ) -> BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async { panic!("unexpected fetch") })
    }
}

impl DatabaseDriver for TestDriver {
    /// Exposes the driver wrapper through the database trait contract.
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// Fails if the return path unexpectedly starts a client transaction.
    fn begin(&self) -> BoxFuture<'_, Result<DbTransaction, DbError>> {
        Box::pin(async { panic!("unexpected transaction") })
    }

    /// Fails if these isolated return scenarios start a notification listener.
    fn listen<'a>(
        &'a self,
        _channel: &'a str,
    ) -> BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async { panic!("unexpected listener") })
    }
}

/// Seeds a claimed job without starting unrelated fetch or timer tasks.
async fn queue_with_claim(driver: Arc<ReturnDriver>) -> LocalQueue {
    let (job_signal_sender, _rx) = runtime::channel(1);
    let queue: LocalQueue = LocalQueueParams {
        config: LocalQueueConfig::default(),
        database: Database::new(TestDriver(driver)),
        schema: "graphile_worker".into(),
        worker_id: "return-test".into(),
        task_details: Default::default(),
        poll_interval: Duration::from_secs(1),
        continuous: true,
        shutdown_signal: None,
        hooks: Arc::new(HookRegistry::default()),
        job_signal_sender,
        use_local_time: false,
    }
    .into();
    *queue.0.mode.write().await = LocalQueueMode::Waiting;
    queue.0.run_complete.store(true, Ordering::Release);
    queue
        .0
        .job_queue
        .lock()
        .await
        .push_back(Job::builder().id(1).build());
    queue
}

/// Exhausts the real retry policy under virtual time, then retries through shutdown.
#[tokio::test(start_paused = true)]
async fn ttl_failure_retains_claims_for_release() {
    let driver = Arc::new(ReturnDriver::default());
    driver.fail.store(true, Ordering::SeqCst);
    let queue = queue_with_claim(driver.clone()).await;
    queue.set_mode_ttl_expired().await;
    assert_eq!(driver.calls.load(Ordering::SeqCst), 20);
    assert_eq!(queue.0.pending_returns.lock().await.len(), 1);
    assert!(
        queue.get_job(&[]).await.is_none(),
        "uncertain returns must not reach handlers"
    );
    driver.fail.store(false, Ordering::SeqCst);
    queue.release().await.unwrap();
    assert_eq!(driver.calls.load(Ordering::SeqCst), 21);
    assert!(queue.0.job_queue.lock().await.is_empty());
    assert!(queue.0.pending_returns.lock().await.is_empty());
}

/// Cancelling a TTL return must leave its claims available to awaited shutdown.
#[tokio::test]
async fn cancelled_ttl_return_retains_claims() {
    let driver = Arc::new(ReturnDriver::default());
    driver.block.store(true, Ordering::SeqCst);
    let queue = queue_with_claim(driver.clone()).await;
    {
        let ttl = queue.set_mode_ttl_expired();
        futures::pin_mut!(ttl);
        assert!(futures::poll!(ttl.as_mut()).is_pending());
    }
    assert_eq!(queue.0.pending_returns.lock().await.len(), 1);
    driver.block.store(false, Ordering::SeqCst);
    queue.release().await.unwrap();
    assert_eq!(driver.calls.load(Ordering::SeqCst), 2);
    assert!(queue.0.job_queue.lock().await.is_empty());
    assert!(queue.0.pending_returns.lock().await.is_empty());
}

/// Cancelling a release caller must permit the next caller to finish its return.
#[tokio::test]
async fn cancelled_release_retains_claims() {
    let driver = Arc::new(ReturnDriver::default());
    driver.block.store(true, Ordering::SeqCst);
    let queue = queue_with_claim(driver.clone()).await;
    {
        let release = queue.release();
        futures::pin_mut!(release);
        assert!(futures::poll!(release.as_mut()).is_pending());
    }
    assert_eq!(queue.0.pending_returns.lock().await.len(), 1);
    driver.block.store(false, Ordering::SeqCst);
    queue.release().await.unwrap();
    assert_eq!(driver.calls.load(Ordering::SeqCst), 2);
    assert!(queue.0.job_queue.lock().await.is_empty());
    assert!(queue.0.pending_returns.lock().await.is_empty());
}

/// A consumer waiting behind a return must recheck the terminal shutdown mode.
#[tokio::test]
async fn waiting_consumer_does_not_take_claim_after_release() {
    let driver = Arc::new(ReturnDriver::default());
    let queue = queue_with_claim(driver.clone()).await;
    let cache = queue.0.job_queue.lock().await;
    let mut consumer = Box::pin(queue.get_job(&[]));
    assert!(futures::poll!(consumer.as_mut()).is_pending());
    let mut release = Box::pin(queue.release());
    assert!(futures::poll!(release.as_mut()).is_pending());
    drop(cache);
    assert!(consumer.await.is_none());
    release.await.unwrap();
    assert_eq!(driver.calls.load(Ordering::SeqCst), 1);
}

/// Shutdown retains an interrupted TTL return and any jobs added by an in-flight fetch.
#[tokio::test]
async fn release_returns_ttl_backlog_and_in_flight_fetch() {
    let driver = Arc::new(ReturnDriver::default());
    driver.block.store(true, Ordering::SeqCst);
    let queue = queue_with_claim(driver.clone()).await;
    let ttl_queue = queue.clone();
    queue
        .0
        .ttl_timer_task
        .replace_abort(runtime::spawn(async move {
            ttl_queue.set_mode_ttl_expired().await;
        }));
    tokio::time::timeout(Duration::from_secs(2), async {
        while driver.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("TTL did not start its return");
    queue.0.run_complete.store(false, Ordering::Release);
    let fetch_queue = queue.clone();
    let fetch = runtime::spawn(async move {
        fetch_queue
            .received_jobs(vec![Job::builder().id(2).build()], false)
            .await;
        fetch_queue.0.run_complete.store(true, Ordering::Release);
        fetch_queue.0.run_complete_notify.notify_waiters();
    });
    driver.block.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), queue.release())
        .await
        .expect("release must unblock the fetch before waiting for it")
        .unwrap();
    fetch.await.unwrap();
    assert_eq!(driver.calls.load(Ordering::SeqCst), 2);
    assert!(queue.0.job_queue.lock().await.is_empty());
    assert!(queue.0.pending_returns.lock().await.is_empty());
}
