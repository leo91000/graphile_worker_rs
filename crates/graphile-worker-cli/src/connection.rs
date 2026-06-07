use anyhow::{anyhow, Context, Result};
use graphile_worker::{Database, Schema, WorkerUtils};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::args::Cli;

pub(crate) struct CliConnection {
    pub(crate) pool: PgPool,
    pub(crate) utils: WorkerUtils,
    pub(crate) schema: Schema,
}

pub(crate) async fn connect(cli: &Cli) -> Result<CliConnection> {
    let database_url = cli
        .database_url
        .as_deref()
        .ok_or_else(|| anyhow!("missing database URL; pass --database-url or set DATABASE_URL"))?;
    let schema = Schema::new(&cli.schema);

    let pool = PgPoolOptions::new()
        .max_connections(cli.max_connections)
        .connect(database_url)
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
