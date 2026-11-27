use anyhow::{anyhow, Context, Result};
use graphile_worker::{Database, Schema, WorkerUtils};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;

use crate::args::Cli;

pub(crate) struct CliConnection {
    pub(crate) pool: PgPool,
    pub(crate) utils: WorkerUtils,
    pub(crate) schema: Schema,
}

pub(crate) async fn connect(cli: &Cli) -> Result<CliConnection> {
    let schema = Schema::new(&cli.schema);
    let options = match cli.database_url.as_deref() {
        Some(url) => url.parse().context("invalid PostgreSQL connection URL")?,
        None => {
            let options = PgConnectOptions::new();
            if options.get_database().is_none_or(str::is_empty) {
                return Err(anyhow!(
                    "missing database configuration; pass --database-url, set DATABASE_URL, or set PGDATABASE and related PostgreSQL variables"
                ));
            }
            options
        }
    };
    let pool = PgPoolOptions::new()
        .max_connections(cli.max_connections)
        .connect_with(options)
        .await
        .context("failed to connect to PostgreSQL")?;

    let database: Database = pool.clone().into();
    let utils = WorkerUtils::new(database, schema.clone());

    Ok(CliConnection {
        pool,
        utils,
        schema,
    })
}
