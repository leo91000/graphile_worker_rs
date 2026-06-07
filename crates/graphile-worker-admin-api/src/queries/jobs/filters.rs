use sqlx::{Postgres, QueryBuilder};

use crate::jobs::{JobState, ListJobsParams};

pub fn apply_job_filters(query: &mut QueryBuilder<Postgres>, args: &ListJobsParams) {
    if let Some(identifier) = args.identifier.as_ref().filter(|value| !value.is_empty()) {
        query.push(" and tasks.identifier = ");
        query.push_bind(identifier);
    }

    if let Some(queue) = args.queue.as_ref().filter(|value| !value.is_empty()) {
        query.push(" and job_queues.queue_name = ");
        query.push_bind(queue);
    }

    apply_search_filter(query, args);
    apply_state_filter(query, args.state);
}

fn apply_search_filter(query: &mut QueryBuilder<Postgres>, args: &ListJobsParams) {
    let Some(search) = args
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    let pattern = format!("%{search}%");
    query.push(" and (jobs.id::text ilike ");
    query.push_bind(pattern.clone());
    query.push(" or tasks.identifier ilike ");
    query.push_bind(pattern.clone());
    query.push(" or coalesce(job_queues.queue_name, '') ilike ");
    query.push_bind(pattern.clone());
    query.push(" or coalesce(jobs.key, '') ilike ");
    query.push_bind(pattern.clone());
    query.push(" or coalesce(jobs.locked_by, '') ilike ");
    query.push_bind(pattern.clone());
    query.push(" or coalesce(jobs.last_error, '') ilike ");
    query.push_bind(pattern.clone());
    query.push(" or jobs.payload::text ilike ");
    query.push_bind(pattern);
    query.push(")");
}

fn apply_state_filter(query: &mut QueryBuilder<Postgres>, state: JobState) {
    match state {
        JobState::All => {}
        JobState::Ready => {
            query.push(
                " and jobs.locked_at is null and jobs.attempts < jobs.max_attempts and jobs.run_at <= now()",
            );
        }
        JobState::Scheduled => {
            query.push(
                " and jobs.locked_at is null and jobs.attempts < jobs.max_attempts and jobs.run_at > now()",
            );
        }
        JobState::Locked => {
            query.push(" and jobs.locked_at is not null");
        }
        JobState::Failed => {
            query.push(" and jobs.locked_at is null and jobs.attempts >= jobs.max_attempts");
        }
    }
}
