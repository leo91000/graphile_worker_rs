use std::collections::HashSet;

use graphile_worker_database::DbExecutor;
use graphile_worker_lifecycle_hooks::{BeforeJobScheduleContext, JobScheduleResult};
use graphile_worker_task_handler::{BatchTaskHandler, TaskHandler};
use indoc::formatdoc;
use serde::Serialize;
use tracing::{debug, Span};

use super::{WorkerUtils, BULK_INSERT_ANALYZE_THRESHOLD};
use crate::sql::add_job::{add_job as insert_job, add_jobs as insert_jobs, JobToAdd, RawJobSpec};
use crate::sql::dynamic::{DynamicSchema, PrivateTable};
use crate::sql::task_identifiers::get_tasks_details;
use crate::tracing::add_tracing_info;
use crate::{errors::GraphileWorkerError, Job, JobSpec};

pub(super) async fn add_job<T: TaskHandler>(
    utils: &WorkerUtils,
    payload: T,
    spec: JobSpec,
) -> Result<Job, GraphileWorkerError> {
    let identifier = T::IDENTIFIER;
    let mut payload = serde_json::to_value(payload)?;
    add_tracing_info(&mut payload);

    let span = Span::current();
    span.record("otel.name", identifier);
    span.record("messaging.destination.name", identifier);

    let payload = invoke_before_job_schedule(utils, identifier, payload, &spec).await?;
    insert_job(
        &utils.database,
        &utils.escaped_schema,
        identifier,
        payload,
        spec,
        utils.use_local_time,
    )
    .await
}

pub(super) async fn add_raw_job<P>(
    utils: &WorkerUtils,
    identifier: &str,
    payload: P,
    spec: JobSpec,
) -> Result<Job, GraphileWorkerError>
where
    P: Serialize,
{
    let mut payload = serde_json::to_value(payload)?;
    add_tracing_info(&mut payload);

    let payload = invoke_before_job_schedule(utils, identifier, payload, &spec).await?;
    insert_job(
        &utils.database,
        &utils.escaped_schema,
        identifier,
        payload,
        spec,
        utils.use_local_time,
    )
    .await
}

pub(super) async fn add_jobs<T: TaskHandler + Clone>(
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
        &utils.escaped_schema,
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

pub(super) async fn add_raw_jobs(
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

    let mut seen_identifiers = HashSet::with_capacity(jobs.len());
    let mut unique_identifiers = Vec::new();
    for job in jobs {
        if seen_identifiers.insert(job.identifier.as_str()) {
            unique_identifiers.push(job.identifier.clone());
        }
    }

    let task_details =
        get_tasks_details(&utils.database, &utils.escaped_schema, unique_identifiers).await?;

    let added_jobs = insert_jobs(
        &utils.database,
        &utils.escaped_schema,
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

pub(super) async fn add_batch_job<T: BatchTaskHandler>(
    utils: &WorkerUtils,
    payloads: Vec<T>,
    spec: JobSpec,
) -> Result<Job, GraphileWorkerError> {
    let span = Span::current();
    span.record("messaging.batch.message_count", payloads.len());

    if payloads.is_empty() {
        return Err(GraphileWorkerError::JobScheduleFailed(
            "batch job payload must contain at least one item".to_string(),
        ));
    }

    let identifier = T::IDENTIFIER;
    span.record("otel.name", identifier);
    span.record("messaging.destination.name", identifier);

    let mut payload = serde_json::to_value(payloads)?;
    if let Some(items) = payload.as_array_mut() {
        for item in items {
            add_tracing_info(item);
        }
    }

    let payload = invoke_before_job_schedule(utils, identifier, payload, &spec).await?;

    let serde_json::Value::Array(payloads) = payload else {
        return Err(GraphileWorkerError::JobScheduleFailed(
            "before_job_schedule must return a JSON array for batch jobs".to_string(),
        ));
    };

    if payloads.is_empty() {
        return Err(GraphileWorkerError::JobScheduleFailed(
            "batch job payload must contain at least one item".to_string(),
        ));
    }

    insert_job(
        &utils.database,
        &utils.escaped_schema,
        identifier,
        serde_json::Value::Array(payloads),
        spec,
        utils.use_local_time,
    )
    .await
}

async fn invoke_before_job_schedule(
    utils: &WorkerUtils,
    identifier: &str,
    payload: serde_json::Value,
    spec: &JobSpec,
) -> Result<serde_json::Value, GraphileWorkerError> {
    let Some(hooks) = &utils.hooks else {
        return Ok(payload);
    };

    let ctx = BeforeJobScheduleContext {
        identifier: identifier.to_string(),
        payload,
        spec: spec.clone(),
    };

    match hooks.intercept(ctx).await {
        JobScheduleResult::Continue(payload) => Ok(payload),
        JobScheduleResult::Skip => Err(GraphileWorkerError::JobScheduleSkipped),
        JobScheduleResult::Fail(msg) => Err(GraphileWorkerError::JobScheduleFailed(msg)),
    }
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

async fn analyze_jobs_after_large_batch(utils: &WorkerUtils, job_count: usize) {
    if job_count < BULK_INSERT_ANALYZE_THRESHOLD {
        return;
    }

    let jobs = DynamicSchema::new(&utils.escaped_schema).private_table(PrivateTable::Jobs);
    let sql = formatdoc!(
        r#"
            analyze {jobs};
        "#
    );

    if let Err(error) = utils
        .database
        .execute(&sql, graphile_worker_database::DbParams::new())
        .await
    {
        debug!(?error, "Failed to analyze jobs after large batch insert");
    }
}
