#![cfg(feature = "runtime-tokio")]

use std::any::Any;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, TimeZone, Utc};
use futures::StreamExt;
use graphile_worker_database::{
    row_mapping, Database, DatabaseDriver, DbCell, DbError, DbExecutor, DbParams, DbRow,
    DbTransaction, DbValue, NotificationStream, TransactionDriver,
};
use serde_json::json;

#[cfg(feature = "driver-sqlx")]
use graphile_worker_database::sqlx::SqlxDatabase;
#[cfg(feature = "driver-tokio-postgres")]
use graphile_worker_database::tokio_postgres::TokioPostgresDatabase;

#[path = "driver_contract/support.rs"]
mod support;
use support::*;

#[path = "driver_contract/drivers.rs"]
mod drivers;
#[path = "driver_contract/row_cells.rs"]
mod row_cells;
#[path = "driver_contract/wrapper.rs"]
mod wrapper;
