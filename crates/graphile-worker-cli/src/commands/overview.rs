use anyhow::{Context, Result};
use graphile_worker::Schema;
use graphile_worker_admin_api::queries::{overview, workers as worker_queries};
use sqlx::PgPool;

use crate::args::Cli;
use crate::output::{print_json, print_queues_table, print_workers_table};

pub(crate) async fn stats(cli: &Cli, pool: &PgPool, schema: &Schema) -> Result<()> {
    let stats = overview::get_stats(pool, schema)
        .await
        .context("failed to get stats")?;

    if cli.json {
        print_json(&stats)?;
    } else {
        println!("total\tready\tscheduled\tlocked\tfailed");
        println!(
            "{}\t{}\t{}\t{}\t{}",
            stats.total, stats.ready, stats.scheduled, stats.locked, stats.failed
        );
    }

    Ok(())
}

pub(crate) async fn queues(cli: &Cli, pool: &PgPool, schema: &Schema) -> Result<()> {
    let queues = overview::list_queues(pool, schema)
        .await
        .context("failed to list queues")?;

    if cli.json {
        print_json(&queues)?;
    } else {
        print_queues_table(&queues);
    }

    Ok(())
}

pub(crate) async fn workers(cli: &Cli, pool: &PgPool, schema: &Schema) -> Result<()> {
    let workers = worker_queries::list_locked_workers(pool, schema)
        .await
        .context("failed to list workers")?;

    if cli.json {
        print_json(&workers)?;
    } else {
        print_workers_table(&workers);
    }

    Ok(())
}
