use std::any::Any;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::*;
use crate::{
    Database, DatabaseDriver, DbExecutor, DbParams, DbRow, DbTransaction, DbValue,
    NotificationStream, TransactionDriver,
};

type RecordedCalls = Arc<Mutex<Vec<(tokio::time::Instant, String, DbParams)>>>;

#[derive(Clone, Debug)]
struct Scripted {
    results: Arc<Mutex<VecDeque<Result<(), DbError>>>>,
    calls: RecordedCalls,
}

impl Scripted {
    /// Returns scripted errors before success, recording each execution and bind.
    fn new(results: Vec<Result<(), DbError>>) -> Self {
        Self {
            results: Arc::new(Mutex::new(results.into())),
            calls: Default::default(),
        }
    }

    /// Records requests for verifying retry count, timing and parameter preservation.
    fn call(&self, sql: &str, params: DbParams) -> Result<(), DbError> {
        self.calls
            .lock()
            .unwrap()
            .push((tokio::time::Instant::now(), sql.into(), params));
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected replay")
    }
}

impl DbExecutor for Scripted {
    fn execute<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<u64, DbError>> {
        Box::pin(async move { self.call(sql, params).map(|()| 1) })
    }
    fn fetch_all<'a>(
        &'a self,
        sql: &'a str,
        params: DbParams,
    ) -> crate::BoxFuture<'a, Result<Vec<DbRow>, DbError>> {
        Box::pin(async move { self.call(sql, params).map(|()| Vec::new()) })
    }
}

impl DatabaseDriver for Scripted {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn begin<'a>(&'a self) -> crate::BoxFuture<'a, Result<DbTransaction, DbError>> {
        Box::pin(async move { Ok(DbTransaction::new(Box::new(self.clone()))) })
    }
    fn listen<'a>(
        &'a self,
        channel: &'a str,
    ) -> crate::BoxFuture<'a, Result<Option<NotificationStream>, DbError>> {
        Box::pin(async move {
            self.call(channel, DbParams::new())?;
            Ok(None)
        })
    }
}

impl TransactionDriver for Scripted {
    fn commit(self: Box<Self>) -> crate::BoxFuture<'static, Result<(), DbError>> {
        Box::pin(async move { self.call("commit", DbParams::new()) })
    }
}

/// Both execution methods preserve binds, pool identity and exponential jittered backoff.
#[tokio::test(start_paused = true)]
async fn pooled_retries_preserve_parameters_and_driver() {
    for fetch in [false, true] {
        let driver = Scripted::new(vec![
            Err(DbError::with_code("serialization", "40001")),
            Err(DbError::with_code("deadlock", "40P01")),
            Ok(()),
        ]);
        let database = Database::new(driver.clone()).with_retry_policy(SqlRetryPolicy::default());
        assert!(database.clone().downcast_ref::<Scripted>().is_some());
        let start = tokio::time::Instant::now();
        let params = vec![DbValue::Text("same-bind".into())].into();
        if fetch {
            database.fetch_all("statement", params).await.unwrap();
        } else {
            database.execute("statement", params).await.unwrap();
        }
        let calls = driver.calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert!(
            (Duration::from_millis(25)..=Duration::from_millis(76)).contains(&(calls[1].0 - start))
        );
        let second_delay = calls[2].0 - calls[1].0;
        assert!((Duration::from_millis(37)..=Duration::from_millis(114)).contains(&second_delay));
        for (_, sql, params) in calls.iter() {
            assert_eq!(sql, "statement");
            assert!(matches!(&params.values()[0], DbValue::Text(value) if value == "same-bind"));
        }
    }
}

/// Exhaustion returns the last diagnostic and does not sleep after the last attempt.
#[tokio::test(start_paused = true)]
async fn exhausted_policy_returns_last_error_without_final_delay() {
    let driver = Scripted::new(vec![
        Err(DbError::with_code("first", "40001")),
        Err(DbError::with_code("last", "40P01")),
    ]);
    let database = Database::new(driver.clone())
        .with_retry_policy(SqlRetryPolicy::new(NonZeroU32::new(2).unwrap()));
    let start = tokio::time::Instant::now();
    let error = database
        .execute("statement", DbParams::new())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("40P01"));
    assert_eq!(error.to_string(), "last");
    assert_eq!(driver.calls.lock().unwrap().len(), 2);
    assert_eq!(
        tokio::time::Instant::now(),
        driver.calls.lock().unwrap().last().unwrap().0
    );
    assert!(tokio::time::Instant::now() - start < Duration::from_millis(77));
    assert_eq!(SqlRetryPolicy::default().max_attempts().get(), 100);
}

/// Connection acquisition failures use the longer delay; unknown outcomes never replay.
#[tokio::test(start_paused = true)]
async fn connection_retries_require_known_pre_execution_failure() {
    for error in [
        DbError::with_code("starting up", "57P03"),
        DbError::connection_failure("unreachable", "EHOSTUNREACH"),
        DbError::connection_failure("timeout", "ETIMEDOUT"),
    ] {
        let driver = Scripted::new(vec![Err(error), Ok(())]);
        let start = tokio::time::Instant::now();
        Database::new(driver)
            .with_retry_policy(SqlRetryPolicy::default())
            .fetch_all("statement", DbParams::new())
            .await
            .unwrap();
        assert!((Duration::from_millis(1500)..=Duration::from_millis(4501))
            .contains(&(tokio::time::Instant::now() - start)));
    }
    for error in [
        DbError::new("connection lost after sending"),
        DbError::with_code("unknown timeout", "ETIMEDOUT"),
        DbError::with_code("unknown unreachable", "EHOSTUNREACH"),
        DbError::with_code("syntax", "42601"),
        DbError::with_code("unique", "23505"),
    ] {
        let expected = error.to_string();
        let driver = Scripted::new(vec![Err(error)]);
        let start = tokio::time::Instant::now();
        let error = Database::new(driver.clone())
            .with_retry_policy(SqlRetryPolicy::default())
            .execute("statement", DbParams::new())
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert_eq!(driver.calls.lock().unwrap().len(), 1);
        assert_eq!(tokio::time::Instant::now(), start);
    }
}

/// Raw handles, transactions, commits and listeners retain immediate error semantics.
#[tokio::test(start_paused = true)]
async fn retries_are_opt_in_and_do_not_replay_transactions_or_listen() {
    let error = || Err(DbError::with_code("original", "40001"));
    let driver = Scripted::new(vec![error(), error(), error(), error(), error()]);
    let original = Database::new(driver.clone());
    let retrying = original
        .clone()
        .with_retry_policy(SqlRetryPolicy::default());
    original.execute("raw", DbParams::new()).await.unwrap_err();
    let transaction = retrying.begin().await.unwrap();
    transaction
        .execute("transaction", DbParams::new())
        .await
        .unwrap_err();
    transaction
        .fetch_all("transaction", DbParams::new())
        .await
        .unwrap_err();
    transaction.commit().await.unwrap_err();
    assert!(retrying.listen("jobs:insert").await.is_err());
    assert_eq!(driver.calls.lock().unwrap().len(), 5);
}

/// Dropping a pending retry stops further database attempts.
#[tokio::test(start_paused = true)]
async fn cancellation_during_backoff_stops_replays() {
    let driver = Scripted::new(vec![Err(DbError::with_code("serialization", "40001"))]);
    let database = Database::new(driver.clone()).with_retry_policy(SqlRetryPolicy::default());
    let task = tokio::spawn(async move { database.execute("statement", DbParams::new()).await });
    tokio::task::yield_now().await;
    assert_eq!(driver.calls.lock().unwrap().len(), 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::advance(Duration::from_secs(10)).await;
    assert_eq!(driver.calls.lock().unwrap().len(), 1);
}

/// Driver conversion classifies only failures before pool acquisition succeeds.
#[test]
fn acquisition_classification_keeps_ambiguous_execution_errors_unmarked() {
    assert_eq!(
        DbError::connection_io_code(&std::io::Error::from(std::io::ErrorKind::HostUnreachable)),
        Some("EHOSTUNREACH")
    );
    assert_eq!(
        DbError::connection_io_code(&std::io::Error::from(std::io::ErrorKind::TimedOut)),
        Some("ETIMEDOUT")
    );
    assert_eq!(
        DbError::connection_io_code(&std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
        None
    );
    #[cfg(feature = "driver-sqlx")]
    {
        let error = crate::sqlx::driver::acquisition_error(::sqlx::Error::PoolTimedOut);
        assert_eq!(error.code(), Some("ETIMEDOUT"));
        assert!(error.is_connection_failure());
        let error: DbError =
            ::sqlx::Error::Io(std::io::Error::from(std::io::ErrorKind::TimedOut)).into();
        assert!(!error.is_connection_failure());
        assert!(SqlRetryPolicy::backoff(&error, 0, 0.5).is_none());
    }
    #[cfg(feature = "driver-tokio-postgres")]
    {
        let error: DbError =
            deadpool_postgres::PoolError::Timeout(deadpool_postgres::TimeoutType::Wait).into();
        assert_eq!(error.code(), Some("ETIMEDOUT"));
        assert!(error.is_connection_failure());
    }
}

/// Pins the current upstream exponential calculator at zero, midpoint and capped jitter.
#[test]
fn delay_matches_upstream_calculator_at_pinned_jitter() {
    for (code, min_ms, cap_ms) in [
        ("40001", 50, 5000),
        ("40P01", 50, 5000),
        ("57P03", 3000, 120000),
    ] {
        let error = DbError::with_code("fault", code);
        for (previous, base) in [
            (0, min_ms as f64),
            (1, min_ms as f64 * 1.5),
            (1000, cap_ms as f64),
        ] {
            for jitter in [0.0, 0.5, 1.0] {
                assert_eq!(
                    SqlRetryPolicy::backoff(&error, previous, jitter).unwrap(),
                    Duration::from_secs_f64(base * (0.5 + jitter) / 1000.0)
                );
            }
        }
    }
}

/// Disabled policies and unclassified pool failures perform one immediate attempt.
#[tokio::test(start_paused = true)]
async fn one_attempt_and_unclassified_acquisition_errors_do_not_replay() {
    let driver = Scripted::new(vec![Err(DbError::with_code("single", "40001"))]);
    let error = Database::new(driver.clone())
        .with_retry_policy(SqlRetryPolicy::new(NonZeroU32::new(1).unwrap()))
        .fetch_all("statement", DbParams::new())
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "single");
    assert_eq!(driver.calls.lock().unwrap().len(), 1);
    assert_eq!(
        DbError::connection_io_code(&DbError::new("no I/O cause")),
        None
    );
    #[cfg(feature = "driver-sqlx")]
    {
        let error = crate::sqlx::driver::acquisition_error(::sqlx::Error::Io(
            std::io::Error::from(std::io::ErrorKind::HostUnreachable),
        ));
        assert_eq!(error.code(), Some("EHOSTUNREACH"));
        assert!(error.is_connection_failure());
        let error = crate::sqlx::driver::acquisition_error(::sqlx::Error::PoolClosed);
        assert!(!error.is_connection_failure());
        assert_eq!(error.code(), None);
    }
    #[cfg(feature = "driver-tokio-postgres")]
    {
        let error: DbError = deadpool_postgres::PoolError::Closed.into();
        assert!(!error.is_connection_failure());
        assert_eq!(error.code(), None);
    }
}
