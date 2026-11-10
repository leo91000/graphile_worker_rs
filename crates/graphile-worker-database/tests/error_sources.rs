#![cfg(feature = "driver-tokio-postgres")]

use std::error::Error;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use graphile_worker_database::tokio_postgres::TokioPostgresDatabase;
use graphile_worker_database::DbError;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Finds native errors through the public source chain, including shared wrappers.
fn native_source<T: Error + 'static>(error: &DbError) -> &T {
    let mut source: &(dyn Error + 'static) = error;
    loop {
        if let Some(native) = source.downcast_ref::<T>() {
            return native;
        }
        source = source.source().expect("native error source retained");
    }
}

/// URL parsing failures retain their original driver error after cloning.
#[test]
fn invalid_url_retains_native_source() {
    let error =
        TokioPostgresDatabase::from_url("postgres://localhost:invalid/postgres", 1).unwrap_err();
    let clone = error.clone();
    for error in [&error, &clone] {
        assert_eq!(error.code(), None);
        assert!(!error.is_connection_failure());
        assert_eq!(
            error.to_string(),
            native_source::<tokio_postgres::Error>(error).to_string()
        );
    }
    assert!(std::ptr::eq(
        native_source::<tokio_postgres::Error>(&error),
        native_source::<tokio_postgres::Error>(&clone)
    ));
}

/// A deterministic transport failure avoids relying on network routes or timeouts.
#[derive(Debug)]
struct FailingTransport(io::ErrorKind);

impl AsyncRead for FailingTransport {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::Error::new(self.0, "native transport failure")))
    }
}

impl AsyncWrite for FailingTransport {
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, _: &[u8]) -> Poll<io::Result<usize>> {
        Poll::Ready(Err(io::Error::new(self.0, "native transport failure")))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Only acquisition failures are retryable; both paths retain the native I/O cause.
#[tokio::test]
async fn io_sources_survive_conversion_without_changing_retry_classification() {
    for (kind, code) in [
        (io::ErrorKind::HostUnreachable, Some("EHOSTUNREACH")),
        (io::ErrorKind::TimedOut, Some("ETIMEDOUT")),
        (io::ErrorKind::ConnectionReset, None),
    ] {
        for acquisition in [false, true] {
            let mut config = tokio_postgres::Config::new();
            config.user("error_source_test");
            let native = config
                .connect_raw(FailingTransport(kind), tokio_postgres::NoTls)
                .await
                .err()
                .expect("transport must fail");
            let message = native.to_string();
            let error: DbError = if acquisition {
                deadpool_postgres::PoolError::Backend(native).into()
            } else {
                native.into()
            };
            let clone = error.clone();
            for error in [&error, &clone] {
                assert_eq!(error.code(), if acquisition { code } else { None });
                assert_eq!(error.is_connection_failure(), acquisition && code.is_some());
                assert_eq!(native_source::<io::Error>(error).kind(), kind);
                assert_eq!(
                    native_source::<tokio_postgres::Error>(error).to_string(),
                    message
                );
                assert!(error.source().unwrap().to_string().contains(&message));
            }
            assert!(std::ptr::eq(
                native_source::<io::Error>(&error),
                native_source::<io::Error>(&clone)
            ));
        }
    }
}

/// A subprocess isolates PGSSLROOTCERT from concurrently running connection tests.
#[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
#[test]
fn tls_setup_failure_retains_native_source() {
    const CHILD_ENV: &str = "GRAPHILE_WORKER_TEST_TLS_ERROR_SOURCE";
    if std::env::var_os(CHILD_ENV).is_none() {
        let missing_roots = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/nonexistent-error-source-roots.pem");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tls_setup_failure_retains_native_source",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .env("PGSSLROOTCERT", missing_roots)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }

    let error = TokioPostgresDatabase::from_config(tokio_postgres::Config::new(), 1).unwrap_err();
    let clone = error.clone();
    for error in [&error, &clone] {
        assert_eq!(error.code(), None);
        assert!(!error.is_connection_failure());
        assert_eq!(
            native_source::<io::Error>(error).kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(error.source().unwrap().to_string(), error.to_string());
        assert!(matches!(
            native_source::<graphile_worker_postgres_tls::Error>(error),
            graphile_worker_postgres_tls::Error::ReadRoots(_)
        ));
    }
}
