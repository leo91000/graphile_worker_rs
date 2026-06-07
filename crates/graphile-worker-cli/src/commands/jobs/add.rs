use std::fs;

use anyhow::{Context, Result};
use graphile_worker::{JobSpec, WorkerUtils};
use serde_json::Value;

use crate::args::{AddArgs, Cli};
use crate::output::{db_job_output_from_job, print_json};

pub(crate) async fn add(cli: &Cli, args: &AddArgs, utils: &WorkerUtils) -> Result<()> {
    let payload = read_payload(args)?;
    let spec = JobSpec {
        queue_name: args.queue.clone(),
        run_at: args.run_at,
        max_attempts: args.max_attempts,
        job_key: args.key.clone(),
        job_key_mode: args.job_key_mode.map(Into::into),
        priority: args.priority,
        flags: (!args.flags.is_empty()).then(|| args.flags.clone()),
    };
    let job = utils
        .add_raw_job(&args.identifier, payload, spec)
        .await
        .context("failed to add job")?;

    if cli.json {
        print_json(&db_job_output_from_job(&job))?;
    } else {
        println!(
            "Added job {} for task `{}`",
            job.id(),
            job.task_identifier()
        );
    }

    Ok(())
}

fn read_payload(args: &AddArgs) -> Result<Value> {
    let payload = match (&args.payload, &args.payload_file) {
        (Some(payload), None) => payload.clone(),
        (None, Some(path)) => fs::read_to_string(path)
            .with_context(|| format!("failed to read payload file `{}`", path.display()))?,
        (None, None) => "{}".to_string(),
        (Some(_), Some(_)) => unreachable!("clap prevents payload and payload_file together"),
    };

    serde_json::from_str(&payload).context("payload must be valid JSON")
}
