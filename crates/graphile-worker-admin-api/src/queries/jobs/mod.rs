use graphile_worker_database::Schema;
use indoc::formatdoc;
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::error::{AdminQueryError, Result};
use crate::jobs::{ListJobsParams, ListedJob};

pub mod filters;
mod select;
pub mod tasks;

use select::jobs_select_sql;

#[derive(Debug, Clone, Copy, Default)]
pub struct ListJobsQueryOptions {
    pub max_limit: Option<i64>,
}

pub async fn list_jobs(
    pool: &PgPool,
    schema: &Schema,
    args: &ListJobsParams,
    options: ListJobsQueryOptions,
) -> Result<Vec<ListedJob>> {
    if args.limit < 0 {
        return Err(AdminQueryError::bad_request(
            "limit must be greater than or equal to 0",
        ));
    }
    if args.offset < 0 {
        return Err(AdminQueryError::bad_request(
            "offset must be greater than or equal to 0",
        ));
    }

    let limit = options
        .max_limit
        .map_or(args.limit, |max_limit| args.limit.min(max_limit));
    let select_sql = jobs_select_sql(schema);
    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            {select_sql}
            where true
        "#
    ));

    filters::apply_job_filters(&mut query, args);
    query.push(" order by jobs.id asc limit ");
    query.push_bind(limit);
    query.push(" offset ");
    query.push_bind(args.offset);

    query
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

pub async fn get_job(pool: &PgPool, schema: &Schema, id: i64) -> Result<ListedJob> {
    let select_sql = jobs_select_sql(schema);
    let mut query = QueryBuilder::<Postgres>::new(formatdoc!(
        r#"
            {select_sql}
            where jobs.id =
        "#
    ));
    query.push_bind(id);

    query
        .build_query_as()
        .fetch_one(pool)
        .await
        .map_err(|error| match error {
            sqlx::Error::RowNotFound => AdminQueryError::not_found(format!("job {id} not found")),
            error => error.into(),
        })
}
