//! Shared admin API contracts for Graphile Worker.
//!
//! Request and response DTOs are available without database features so native
//! and WASM admin clients can share one wire contract. The optional `sqlx`
//! feature exposes read-query helpers for the native admin server and CLI; those
//! helpers are SQLx-backed by design and are not the core driver's abstraction.

pub mod defaults;
pub mod jobs;
pub mod maintenance;
pub mod overview;
pub mod responses;

#[cfg(feature = "sqlx")]
pub mod queries;
#[cfg(feature = "sqlx")]
mod sql;

#[cfg(test)]
mod tests;
