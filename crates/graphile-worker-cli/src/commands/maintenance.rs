use anyhow::{Context, Result};
use graphile_worker::worker_utils::types::CleanupTask;
use graphile_worker::{SweepStaleWorkersOptions, WorkerUtils};

use crate::args::{CleanupArgs, CleanupTaskArg, Cli, ForceUnlockArgs, SweepStaleWorkersArgs};
use crate::output::print_json;

pub(crate) async fn cleanup(cli: &Cli, args: &CleanupArgs, utils: &WorkerUtils) -> Result<()> {
    let requested_tasks = if args.tasks.is_empty() {
        CleanupTaskArg::all()
    } else {
        args.tasks.clone()
    };
    let tasks: Vec<CleanupTask> = requested_tasks.iter().copied().map(Into::into).collect();
    utils.cleanup(&tasks).await.context("failed to cleanup")?;

    if cli.json {
        print_json(&serde_json::json!({ "cleanup_tasks": requested_tasks }))?;
    } else {
        println!("Cleanup complete");
    }

    Ok(())
}

pub(crate) async fn force_unlock(
    cli: &Cli,
    args: &ForceUnlockArgs,
    utils: &WorkerUtils,
) -> Result<()> {
    let worker_ids: Vec<&str> = args.worker_ids.iter().map(String::as_str).collect();
    utils
        .force_unlock_workers(&worker_ids)
        .await
        .context("failed to force unlock workers")?;

    if cli.json {
        print_json(&serde_json::json!({ "unlocked_workers": args.worker_ids }))?;
    } else {
        println!("Unlocked {} worker id(s)", args.worker_ids.len());
    }

    Ok(())
}

pub(crate) async fn migrate(cli: &Cli, utils: &WorkerUtils) -> Result<()> {
    utils.migrate().await.context("failed to run migrations")?;

    if cli.json {
        print_json(&serde_json::json!({ "migrated": true }))?;
    } else {
        println!("Migrations complete");
    }

    Ok(())
}

pub(crate) async fn sweep_stale_workers(
    cli: &Cli,
    args: &SweepStaleWorkersArgs,
    utils: &WorkerUtils,
) -> Result<()> {
    let result = utils
        .sweep_stale_workers(SweepStaleWorkersOptions {
            sweep_threshold: args.sweep_threshold,
            recovery_delay: args.recovery_delay,
            dry_run: args.dry_run,
        })
        .await
        .context("failed to sweep stale workers")?;

    if cli.json {
        print_json(&result)?;
    } else if args.dry_run {
        if result.worker_ids.is_empty() {
            println!("No stale workers found");
        } else {
            println!(
                "Would recover {} worker(s): {}",
                result.worker_ids.len(),
                result.worker_ids.join(", ")
            );
        }
    } else if result.worker_ids.is_empty() {
        println!("No stale workers found");
    } else {
        println!(
            "Recovered {} job(s) from {} worker(s): {}",
            result.recovered_count,
            result.worker_ids.len(),
            result.worker_ids.join(", ")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_cleanup_task_names() {
        let tasks: Vec<CleanupTask> = CleanupTaskArg::all().into_iter().map(Into::into).collect();

        assert_eq!(tasks.len(), 3);
    }
}
