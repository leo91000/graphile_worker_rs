use thiserror::Error;

#[derive(Debug, Error, Clone)]
#[error("{message}")]
pub struct DbError {
    message: String,
    code: Option<String>,
    connection_failure: bool,
}

impl DbError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
            connection_failure: false,
        }
    }

    pub fn with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: Some(code.into()),
            connection_failure: false,
        }
    }

    /// Marks a failure that occurred before a statement could be sent.
    /// Only driver pool-acquisition paths may call this constructor.
    #[cfg(any(feature = "driver-sqlx", feature = "driver-tokio-postgres", test))]
    pub(crate) fn connection_failure(message: impl Into<String>, code: &'static str) -> Self {
        Self {
            message: message.into(),
            code: Some(code.into()),
            connection_failure: true,
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

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }
}
