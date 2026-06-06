mod completion;
mod failure;
mod shared;

#[cfg(all(test, feature = "driver-sqlx"))]
mod tests;

pub use completion::{CompletionBatcher, CompletionRequest};
pub use failure::{FailureBatcher, FailureRequest};

const BATCHER_CHANNEL_CAPACITY: usize = 4096;
