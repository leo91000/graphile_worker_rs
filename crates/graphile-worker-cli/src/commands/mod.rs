mod admin_server;
mod jobs;
mod maintenance;
mod overview;

use anyhow::Result;
use graphile_worker::{Schema, WorkerUtils};
use sqlx::PgPool;

use crate::args::{Cli, Command};

pub(crate) async fn run_command(
    cli: &Cli,
    pool: &PgPool,
    utils: &WorkerUtils,
    schema: &Schema,
) -> Result<()> {
    match &cli.command {
        Command::Add(args) => jobs::add::add(cli, args, utils).await?,
        Command::List(args) => jobs::list::list(cli, args, pool, schema).await?,
        Command::Show(args) => jobs::list::show(cli, args, pool, schema).await?,
        Command::Complete(args) => jobs::actions::complete(cli, args, utils).await?,
        Command::Fail(args) => jobs::actions::fail(cli, args, utils).await?,
        Command::Reschedule(args) => jobs::actions::reschedule(cli, args, utils).await?,
        Command::Remove(args) => jobs::actions::remove(cli, args, utils).await?,
        Command::Cleanup(args) => maintenance::cleanup(cli, args, utils).await?,
        Command::ForceUnlock(args) => maintenance::force_unlock(cli, args, utils).await?,
        Command::Migrate => maintenance::migrate(cli, utils).await?,
        Command::Stats => overview::stats(cli, pool, schema).await?,
        Command::Queues => overview::queues(cli, pool, schema).await?,
        Command::Workers => overview::workers(cli, pool, schema).await?,
        Command::SweepStaleWorkers(args) => {
            maintenance::sweep_stale_workers(cli, args, utils).await?
        }
        Command::Admin(args) => admin_server::serve(cli, args, pool, utils, schema).await?,
    }

    Ok(())
}
