use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
#[error("{message}")]
pub struct DbError {
    message: String,
    code: DatabaseErrorCode,
    connection_failure: bool,
    #[source]
    source: Option<Arc<NativeErrorSource>>,
}

impl DbError {
    /// Creates an unclassified error whose execution outcome must not be assumed.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: DatabaseErrorCode::Unavailable,
            connection_failure: false,
            source: None,
        }
    }

    /// Preserves a server code without asserting a pre-execution connection failure.
    pub fn with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: DatabaseErrorCode::Reported(code.into()),
            connection_failure: false,
            source: None,
        }
    }

    /// Marks a failure that occurred before a statement could be sent.
    /// Only driver pool-acquisition paths may call this constructor.
    #[cfg(any(feature = "driver-sqlx", feature = "driver-tokio-postgres", test))]
    pub(crate) fn connection_failure(message: impl Into<String>, code: &'static str) -> Self {
        Self {
            message: message.into(),
            code: DatabaseErrorCode::Reported(code.into()),
            connection_failure: true,
            source: None,
        }
    }

    /// Whether a driver proved this error happened before sending a statement.
    pub fn is_connection_failure(&self) -> bool {
        self.connection_failure
    }

    /// Classifies the I/O errors for which upstream retries connection acquisition.
    #[cfg(any(feature = "driver-sqlx", feature = "driver-tokio-postgres", test))]
    pub(crate) fn connection_io_code(
        error: &(dyn std::error::Error + 'static),
    ) -> Option<&'static str> {
        let mut source = Some(error);
        while let Some(error) = source {
            if let Some(error) = error.downcast_ref::<std::io::Error>() {
                match error.kind() {
                    std::io::ErrorKind::HostUnreachable => return Some("EHOSTUNREACH"),
                    std::io::ErrorKind::TimedOut => return Some("ETIMEDOUT"),
                    _ => {}
                }
            }
            source = error.source();
        }
        None
    }

    /// Reads the native server diagnostic before attaching its owned source.
    #[cfg(feature = "driver-tokio-postgres")]
    pub(crate) fn tokio_postgres_diagnostic(error: &::tokio_postgres::Error) -> Self {
        match error.as_db_error() {
            Some(server_error) => Self::with_code(
                format!("{error}: {server_error}"),
                server_error.code().code(),
            ),
            None => Self::new(error.to_string()),
        }
    }

    /// Constructs an infrastructure failure from the original cause rather than a detached message.
    #[cfg(feature = "driver-tokio-postgres")]
    pub(crate) fn from_source(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        let message = source.to_string();
        let diagnostic = Self::new(message);
        diagnostic.with_source(source)
    }

    /// Attaches the native driver cause while keeping cloned errors associated with the same failure.
    #[cfg(feature = "driver-tokio-postgres")]
    pub(crate) fn with_source(
        mut self,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        self.source = Some(Arc::new(NativeErrorSource {
            source: Box::new(source),
        }));
        self
    }

    pub fn code(&self) -> Option<&str> {
        match &self.code {
            DatabaseErrorCode::Unavailable => None,
            DatabaseErrorCode::Reported(code) => Some(code),
        }
    }
}

#[derive(Debug, Error)]
#[error("{source}")]
struct NativeErrorSource {
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
}

#[derive(Debug, Clone)]
enum DatabaseErrorCode {
    Unavailable,
    Reported(String),
}
