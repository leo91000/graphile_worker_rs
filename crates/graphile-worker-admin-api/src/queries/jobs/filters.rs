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

#[cfg(test)]
mod tests {
    use sqlx::Execute;

    use super::*;

    fn params(state: JobState) -> ListJobsParams {
        ListJobsParams {
            state,
            identifier: None,
            queue: None,
            search: None,
            limit: 50,
            offset: 0,
        }
    }

    fn filtered_sql(args: &ListJobsParams) -> String {
        let mut query = QueryBuilder::<Postgres>::new("select * from jobs where true");
        apply_job_filters(&mut query, args);
        query.build().sql().as_str().to_string()
    }

    #[test]
    fn applies_identifier_queue_and_search_filters() {
        let mut args = params(JobState::All);
        args.identifier = Some("send_email".to_string());
        args.queue = Some("emails".to_string());
        args.search = Some("  user@example.com  ".to_string());

        let sql = filtered_sql(&args);

        assert!(sql.contains("tasks.identifier ="));
        assert!(sql.contains("job_queues.queue_name ="));
        assert!(sql.contains("jobs.payload::text ilike"));
    }

    #[test]
    fn ignores_empty_filters() {
        let mut args = params(JobState::All);
        args.identifier = Some(String::new());
        args.queue = Some(String::new());
        args.search = Some("   ".to_string());

        let sql = filtered_sql(&args);

        assert!(!sql.contains("tasks.identifier ="));
        assert!(!sql.contains("job_queues.queue_name ="));
        assert!(!sql.contains("ilike"));
    }

    #[test]
    fn applies_each_state_filter() {
        let cases = [
            (JobState::All, ""),
            (JobState::Ready, "jobs.run_at <= now()"),
            (JobState::Scheduled, "jobs.run_at > now()"),
            (JobState::Locked, "jobs.locked_at is not null"),
            (JobState::Failed, "jobs.attempts >= jobs.max_attempts"),
        ];

        for (state, expected) in cases {
            let sql = filtered_sql(&params(state));
            if expected.is_empty() {
                assert!(!sql.contains("jobs.locked_at"));
            } else {
                assert!(sql.contains(expected), "sql was: {sql}");
            }
        }
    }
}
