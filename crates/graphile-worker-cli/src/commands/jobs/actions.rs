use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use graphile_worker::worker_utils::types::RescheduleJobOptions;
use graphile_worker::WorkerUtils;

use crate::args::{Cli, FailArgs, JobIdsArgs, RemoveArgs, RescheduleArgs};
use crate::output::{print_db_job_result, print_json};

pub(crate) async fn complete(cli: &Cli, args: &JobIdsArgs, utils: &WorkerUtils) -> Result<()> {
    let jobs = utils
        .complete_jobs(&args.ids)
        .await
        .context("failed to complete jobs")?;
    print_db_job_result(cli.json, "Completed", &jobs)
}

pub(crate) async fn fail(cli: &Cli, args: &FailArgs, utils: &WorkerUtils) -> Result<()> {
    let jobs = utils
        .permanently_fail_jobs(&args.ids, &args.reason)
        .await
        .context("failed to permanently fail jobs")?;
    print_db_job_result(cli.json, "Failed", &jobs)
}

pub(crate) async fn reschedule(
    cli: &Cli,
    args: &RescheduleArgs,
    utils: &WorkerUtils,
) -> Result<()> {
    let run_at = if args.now {
        Some(Utc::now())
    } else {
        args.run_at
    };
    if run_at.is_none()
        && args.priority.is_none()
        && args.attempts.is_none()
        && args.max_attempts.is_none()
    {
        return Err(anyhow!(
            "provide at least one of --now, --run-at, --priority, --attempts, or --max-attempts"
        ));
    }

    let jobs = utils
        .reschedule_jobs(
            &args.ids,
            RescheduleJobOptions {
                run_at,
                priority: args.priority,
                attempts: args.attempts,
                max_attempts: args.max_attempts,
            },
        )
        .await
        .context("failed to reschedule jobs")?;
    print_db_job_result(cli.json, "Rescheduled", &jobs)
}

pub(crate) async fn remove(cli: &Cli, args: &RemoveArgs, utils: &WorkerUtils) -> Result<()> {
    utils
        .remove_job(&args.key)
        .await
        .with_context(|| format!("failed to remove job with key `{}`", args.key))?;

    if cli.json {
        print_json(&serde_json::json!({ "removed_key": args.key }))?;
    } else {
        println!("Removed job with key `{}`", args.key);
    }

    Ok(())
}
