use std::collections::HashSet;

use graphile_worker_task_handler::TaskHandler;

use crate::tracing::add_tracing_info;
use crate::{errors::GraphileWorkerError, Job, JobSpec};
use graphile_worker_queries::add_job::batch::add_jobs as insert_jobs;
use graphile_worker_queries::add_job::types::{JobToAdd, RawJobSpec};
use graphile_worker_queries::task_identifiers::get_tasks_details;

use super::super::client::WorkerUtils;
use super::analyze::analyze_jobs_after_large_batch;
use super::hooks::invoke_before_job_schedule;

pub(in crate::worker_utils) async fn add_jobs<T: TaskHandler + Clone>(
    utils: &WorkerUtils,
    jobs: &[(T, &JobSpec)],
) -> Result<Vec<Job>, GraphileWorkerError> {
    if jobs.is_empty() {
        return Ok(vec![]);
    }

    let identifier = T::IDENTIFIER;
    let mut job_inputs = Vec::with_capacity(jobs.len());
    for (payload, spec) in jobs {
        let payload_value = serde_json::to_value(payload)?;
        job_inputs.push((identifier, payload_value, *spec));
    }

    let (jobs_to_add, job_key_preserve_run_at) = prepare_batch_jobs(utils, job_inputs).await?;

    let task_details = utils.task_details.read().await;

    let added_jobs = insert_jobs(
        &utils.database,
        &utils.schema,
        &jobs_to_add,
        &task_details,
        job_key_preserve_run_at,
        utils.use_local_time,
    )
    .await?;
    drop(task_details);

    analyze_jobs_after_large_batch(utils, jobs.len()).await;

    Ok(added_jobs)
}

pub(in crate::worker_utils) async fn add_raw_jobs(
    utils: &WorkerUtils,
    jobs: &[RawJobSpec],
) -> Result<Vec<Job>, GraphileWorkerError> {
    if jobs.is_empty() {
        return Ok(vec![]);
    }

    let job_inputs: Vec<_> = jobs
        .iter()
        .map(|job| (job.identifier.as_str(), job.payload.clone(), &job.spec))
        .collect();

    let (jobs_to_add, job_key_preserve_run_at) = prepare_batch_jobs(utils, job_inputs).await?;
    let task_details =
        get_tasks_details(&utils.database, &utils.schema, unique_identifiers(jobs)).await?;

    let added_jobs = insert_jobs(
        &utils.database,
        &utils.schema,
        &jobs_to_add,
        &task_details,
        job_key_preserve_run_at,
        utils.use_local_time,
    )
    .await?;
    drop(task_details);

    analyze_jobs_after_large_batch(utils, jobs.len()).await;

    Ok(added_jobs)
}

async fn prepare_batch_jobs<'a>(
    utils: &WorkerUtils,
    jobs: Vec<(&'a str, serde_json::Value, &'a JobSpec)>,
) -> Result<(Vec<JobToAdd<'a>>, bool), GraphileWorkerError> {
    let mut jobs_to_add = Vec::with_capacity(jobs.len());
    let mut job_key_preserve_run_at = false;

    for (identifier, mut payload, spec) in jobs {
        add_tracing_info(&mut payload);

        let payload = invoke_before_job_schedule(utils, identifier, payload, spec).await?;

        job_key_preserve_run_at |= spec
            .job_key_mode()
            .as_ref()
            .is_some_and(|m| matches!(m, crate::JobKeyMode::PreserveRunAt));

        jobs_to_add.push(JobToAdd {
            identifier,
            payload,
            spec,
        });
    }

    Ok((jobs_to_add, job_key_preserve_run_at))
}

fn unique_identifiers(jobs: &[RawJobSpec]) -> Vec<String> {
    let mut seen_identifiers = HashSet::with_capacity(jobs.len());
    let mut unique_identifiers = Vec::new();
    for job in jobs {
        if seen_identifiers.insert(job.identifier.as_str()) {
            unique_identifiers.push(job.identifier.clone());
        }
    }
    unique_identifiers
}
