//! Utility functions for adding, managing, and maintaining jobs.
//!
//! `WorkerUtils` is re-exported from the root crate, so applications can use the
//! ergonomic `graphile_worker` API without depending on the internal utility crate.
//!
//! # Example
//!
//! ```no_run
//! use graphile_worker::{JobSpec, WorkerUtils};
//! use serde_json::json;
//!
//! async fn add_email_job(
//!     utils: &WorkerUtils,
//! ) -> Result<(), graphile_worker::errors::GraphileWorkerError> {
//!     let job = utils
//!         .add_raw_job(
//!             "send_email",
//!             json!({ "to": "user@example.com", "subject": "Hello" }),
//!             JobSpec::default(),
//!         )
//!         .await?;
//!
//!     let _ = job;
//!     Ok(())
//! }
//! ```

pub use graphile_worker_utils::{client, types, WorkerUtils};
