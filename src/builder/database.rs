use graphile_worker_database::{Database, DbError};

use super::WorkerOptions;

#[cfg(feature = "driver-sqlx")]
pub(super) async fn connect_default_database(
    db_url: &str,
    max_connections: u32,
) -> Result<Database, DbError> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(db_url)
        .await
        .map_err(DbError::from)?;
    Ok(pool.into())
}

#[cfg(all(not(feature = "driver-sqlx"), feature = "driver-tokio-postgres"))]
pub(super) async fn connect_default_database(
    db_url: &str,
    max_connections: u32,
) -> Result<Database, DbError> {
    let database = graphile_worker_database::tokio_postgres::TokioPostgresDatabase::from_url(
        db_url,
        max_connections as usize,
    )?;
    Ok(database.into())
}

#[cfg(not(any(feature = "driver-sqlx", feature = "driver-tokio-postgres")))]
pub(super) async fn connect_default_database(
    _db_url: &str,
    _max_connections: u32,
) -> Result<Database, DbError> {
    Err(DbError::new(
        "database_url requires enabling a database driver feature",
    ))
}

impl WorkerOptions {
    /// Sets an existing PostgreSQL database connection for the worker to use.
    ///
    /// Allows reusing an existing driver-specific pool or configuring the
    /// database connection with custom settings before passing it to the worker.
    ///
    /// # Arguments
    /// * `value` - The PostgreSQL database connection
    ///
    /// # Note
    /// If both `database` and `database_url` are provided, `database` takes precedence.
    pub fn database(mut self, value: impl Into<Database>) -> Self {
        self.database = Some(value.into());
        self
    }

    /// Sets an existing SQLx PostgreSQL pool for the worker to use.
    ///
    /// This is a SQLx-only convenience wrapper over [`Self::database`]. Prefer
    /// [`Self::database`] when passing a driver-agnostic database connection.
    ///
    /// # Note
    /// If both `pg_pool` and `database_url` are provided, `pg_pool` takes precedence.
    #[cfg(feature = "driver-sqlx")]
    pub fn pg_pool(mut self, value: sqlx::PgPool) -> Self {
        self.database = Some(value.into());
        self
    }

    /// Sets the PostgreSQL database connection URL.
    ///
    /// The URL is used to establish a connection to the database if no
    /// connection pool is provided.
    ///
    /// # Arguments
    /// * `value` - The PostgreSQL connection URL (e.g., "postgres://user:password@localhost/mydb")
    ///
    /// # Note
    /// Either `database`, `pg_pool`, or `database_url` must be provided before calling `init()`.
    pub fn database_url(mut self, value: &str) -> Self {
        self.database_url = Some(value.into());
        self
    }

    /// Sets the maximum number of database connections in the pool.
    ///
    /// Only applies when creating a new connection pool from
    /// a database URL. Ignored if an existing pool is provided.
    ///
    /// # Arguments
    /// * `value` - The maximum number of connections
    ///
    /// # Default
    /// If not specified, defaults to 20 connections.
    pub fn max_pg_conn(mut self, value: u32) -> Self {
        self.max_pg_conn = Some(value);
        self
    }
}
