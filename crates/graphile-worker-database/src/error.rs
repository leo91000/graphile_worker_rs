use thiserror::Error;

#[derive(Debug, Error, Clone)]
#[error("{message}")]
pub struct DbError {
    message: String,
    code: Option<String>,
}

impl DbError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
        }
    }

    pub fn with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: Some(code.into()),
        }
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }
}
