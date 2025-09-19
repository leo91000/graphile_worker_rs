#![doc = include_str!("../README.md")]

//! # Graphile Worker RS
//!
//! A PostgreSQL-backed job queue implementation for Rust applications. This crate is a Rust port
//! of the Node.js [Graphile Worker](https://github.com/graphile/worker) library.
//!
//! ## Architecture Overview
//!
//! Graphile Worker uses PostgreSQL as its backend for job storage and coordination.
//! The system consists of several key components:
//!
//! - **Worker**: Processes jobs from the queue using the specified concurrency.
//! - **WorkerUtils**: Utility functions for job management (adding, removing, rescheduling, etc.).
//! - **TaskHandler**: Trait that defines how specific job types are processed.
//! - **Job Specification**: Configures job parameters like priority, retry behavior, and scheduling.
//! - **Migrations**: Automatic schema management for the database tables.
//!
//! ## Database Schema
//!
//! Graphile Worker manages its own database schema (default: `graphile_worker`).
//! It automatically handles migrations and uses the following tables:
//!
//! - `_private_jobs`: Stores job data, state, and execution metadata
//! - `_private_tasks`: Tracks registered task types
//! - `_private_job_queues`: Manages job queue names for serialized job execution
//! - `_private_workers`: Tracks active worker instances
//!
//! ## Module Structure
//!
//! The crate is organized into the following modules:

/// Configuration and initialization of worker instances
pub mod builder;

/// Error types used throughout the crate
pub mod errors;

/// Job specification and builder for configuring jobs
pub mod job_spec;

/// Core worker implementation for running the job queue
pub mod runner;

/// SQL query implementations for interacting with the database
pub mod sql;

/// Job stream management for processing jobs
pub mod streams;

/// General utility functions
pub mod utils;

pub mod context_ext;
/// Utility functions for job management
pub mod worker_utils;

pub use crate::job_spec::*;
pub use graphile_worker_crontab_parser::parse_crontab;
pub use graphile_worker_ctx::*;
pub use graphile_worker_job::*;
pub use graphile_worker_task_handler::*;

pub use builder::{WorkerBuildError, WorkerOptions};
pub use context_ext::WorkerContextExt;
pub use runner::Worker;
pub use worker_utils::WorkerUtils;
