use anyhow::{anyhow, Context, Result};
use graphile_worker::Schema;
use graphile_worker_admin_api::jobs::{JobState as AdminJobState, ListJobsParams};
use graphile_worker_admin_api::queries::jobs::{self as admin_jobs, ListJobsQueryOptions};
use sqlx::PgPool;

use crate::args::{Cli, CliJobState, ListArgs, ShowArgs};
use crate::output::{print_job_details, print_jobs_table, print_json};

pub(crate) async fn list(cli: &Cli, args: &ListArgs, pool: &PgPool, schema: &Schema) -> Result<()> {
    let params = list_jobs_params(args)?;
    let jobs = admin_jobs::list_jobs(pool, schema, &params, ListJobsQueryOptions::default())
        .await
        .context("failed to list jobs")?;

    if cli.json {
        print_json(&jobs)?;
    } else {
        print_jobs_table(&jobs);
    }

    Ok(())
}

pub(crate) async fn show(cli: &Cli, args: &ShowArgs, pool: &PgPool, schema: &Schema) -> Result<()> {
    let job = admin_jobs::get_job(pool, schema, args.id)
        .await
        .with_context(|| format!("failed to get job {}", args.id))?;

    if cli.json {
        print_json(&job)?;
    } else {
        print_job_details(&job)?;
    }

    Ok(())
}

fn list_jobs_params(args: &ListArgs) -> Result<ListJobsParams> {
    if args.limit < 0 {
        return Err(anyhow!("--limit must be greater than or equal to 0"));
    }
    if args.offset < 0 {
        return Err(anyhow!("--offset must be greater than or equal to 0"));
    }

    Ok(ListJobsParams {
        state: match args.state {
            CliJobState::All => AdminJobState::All,
            CliJobState::Ready => AdminJobState::Ready,
            CliJobState::Scheduled => AdminJobState::Scheduled,
            CliJobState::Locked => AdminJobState::Locked,
            CliJobState::Failed => AdminJobState::Failed,
        },
        identifier: args.identifier.clone(),
        queue: args.queue.clone(),
        search: None,
        limit: args.limit,
        offset: args.offset,
    })
}
