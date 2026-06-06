use anyhow::Result;
use chrono::Utc;
use graphile_worker::DbJob;
use graphile_worker_admin_api::{ListedJob, LockedWorkerRow, QueueRow};
use serde::Serialize;

use crate::db_job_output_from_db_job;

pub(crate) fn print_db_job_result(json: bool, action: &str, jobs: &[DbJob]) -> Result<()> {
    if json {
        let output: Vec<_> = jobs.iter().map(db_job_output_from_db_job).collect();
        print_json(&output)?;
        return Ok(());
    }

    let ids = jobs
        .iter()
        .map(|job| job.id().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    println!("{action} {} job(s): {ids}", jobs.len());
    Ok(())
}

pub(crate) fn print_jobs_table(jobs: &[ListedJob]) {
    println!("id\ttask\tqueue\tstate\trun_at\tattempts\tpriority\tkey\tlocked_by");
    for job in jobs {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}/{}\t{}\t{}\t{}",
            job.id,
            job.task_identifier,
            job.queue_name.as_deref().unwrap_or("-"),
            state_for(job),
            job.run_at.to_rfc3339(),
            job.attempts,
            job.max_attempts,
            job.priority,
            job.key.as_deref().unwrap_or("-"),
            job.locked_by.as_deref().unwrap_or("-")
        );
    }
}

pub(crate) fn print_job_details(job: &ListedJob) -> Result<()> {
    println!("id: {}", job.id);
    println!("task: {}", job.task_identifier);
    println!("queue: {}", job.queue_name.as_deref().unwrap_or("-"));
    println!("state: {}", state_for(job));
    println!("run_at: {}", job.run_at.to_rfc3339());
    println!("attempts: {}/{}", job.attempts, job.max_attempts);
    println!("priority: {}", job.priority);
    println!("key: {}", job.key.as_deref().unwrap_or("-"));
    println!("locked_by: {}", job.locked_by.as_deref().unwrap_or("-"));
    println!("last_error: {}", job.last_error.as_deref().unwrap_or("-"));
    println!("payload: {}", serde_json::to_string_pretty(&job.payload)?);
    Ok(())
}

pub(crate) fn print_queues_table(queues: &[QueueRow]) {
    println!("id\tqueue\tjobs\tready\tlocked_by\tlocked_at");
    for queue in queues {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            queue.id,
            queue.queue_name,
            queue.job_count,
            queue.ready_count,
            queue.locked_by.as_deref().unwrap_or("-"),
            queue
                .locked_at
                .map(|locked_at| locked_at.to_rfc3339())
                .unwrap_or_else(|| "-".to_string())
        );
    }
}

pub(crate) fn print_workers_table(workers: &[LockedWorkerRow]) {
    println!("worker_id\tlocked_jobs\tlocked_queues");
    for worker in workers {
        println!(
            "{}\t{}\t{}",
            worker.worker_id, worker.locked_jobs, worker.locked_queues
        );
    }
}

fn state_for(job: &ListedJob) -> &'static str {
    if job.locked_at.is_some() {
        return "locked";
    }
    if job.attempts >= job.max_attempts {
        return "failed";
    }
    if job.run_at > Utc::now() {
        return "scheduled";
    }
    "ready"
}

pub(crate) fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
