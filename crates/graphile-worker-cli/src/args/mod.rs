mod admin;
mod jobs;
mod maintenance;

use clap::{Parser, Subcommand};

pub(crate) use admin::{AdminArgs, AdminAuthModeArg};
pub(crate) use jobs::{
    AddArgs, CliJobState, FailArgs, JobIdsArgs, ListArgs, RemoveArgs, RescheduleArgs, ShowArgs,
};
pub(crate) use maintenance::{CleanupArgs, CleanupTaskArg, ForceUnlockArgs, SweepStaleWorkersArgs};

#[derive(Parser, Debug)]
#[command(
    name = "graphile-worker",
    about = "Manage Graphile Worker jobs in PostgreSQL",
    version
)]
pub(crate) struct Cli {
    /// PostgreSQL connection URL. Falls back to DATABASE_URL.
    #[arg(long, env = "DATABASE_URL", global = true)]
    pub(crate) database_url: Option<String>,

    /// Graphile Worker schema name. Falls back to GRAPHILE_WORKER_SCHEMA.
    #[arg(
        long,
        env = "GRAPHILE_WORKER_SCHEMA",
        default_value = "graphile_worker",
        global = true
    )]
    pub(crate) schema: String,

    /// Maximum PostgreSQL connections used by the CLI.
    #[arg(long, default_value_t = 5, global = true)]
    pub(crate) max_connections: u32,

    /// Print machine-readable JSON where supported.
    #[arg(long, global = true)]
    pub(crate) json: bool,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Add a raw job to the queue.
    Add(AddArgs),

    /// List jobs, optionally filtered by state, task, or queue.
    List(ListArgs),

    /// Show one job by id.
    Show(ShowArgs),

    /// Mark jobs as completed.
    Complete(JobIdsArgs),

    /// Mark jobs as permanently failed.
    Fail(FailArgs),

    /// Reschedule jobs or update retry metadata.
    Reschedule(RescheduleArgs),

    /// Remove a job by job key.
    Remove(RemoveArgs),

    /// Run maintenance cleanup tasks.
    Cleanup(CleanupArgs),

    /// Force unlock jobs and queues locked by worker ids.
    ForceUnlock(ForceUnlockArgs),

    /// Run Graphile Worker migrations.
    Migrate,

    /// Print queue-wide job counts.
    Stats,

    /// List known queues and their lock state.
    Queues,

    /// List worker ids that currently hold locks.
    Workers,

    /// Recover jobs from inactive workers and orphan locks.
    SweepStaleWorkers(SweepStaleWorkersArgs),

    /// Serve the embedded Leptos admin UI and JSON management API.
    Admin(AdminArgs),
}
