use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArchimedesError {
    #[error("Error occured while query: {0}")]
    SqlError(#[from] sqlx::Error),
    #[error("Error while serializing params: {0}")]
    JsonSerializeError(#[from] serde_json::Error),
}

pub type Result<T> = core::result::Result<T, ArchimedesError>;
