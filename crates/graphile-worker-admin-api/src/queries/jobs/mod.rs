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

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::*;
    use crate::jobs::JobState;

    fn lazy_pool() -> PgPool {
        PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost/postgres")
            .expect("valid database url")
    }

    fn params(limit: i64, offset: i64) -> ListJobsParams {
        ListJobsParams {
            state: JobState::All,
            identifier: None,
            queue: None,
            search: None,
            limit,
            offset,
        }
    }

    #[tokio::test]
    async fn rejects_negative_pagination_before_querying() {
        let pool = lazy_pool();
        let schema = Schema::default();

        let limit_error = list_jobs(
            &pool,
            &schema,
            &params(-1, 0),
            ListJobsQueryOptions::default(),
        )
        .await
        .expect_err("negative limit should fail");
        assert!(matches!(limit_error, AdminQueryError::BadRequest(_)));
        assert_eq!(
            limit_error.to_string(),
            "limit must be greater than or equal to 0"
        );

        let offset_error = list_jobs(
            &pool,
            &schema,
            &params(50, -1),
            ListJobsQueryOptions::default(),
        )
        .await
        .expect_err("negative offset should fail");
        assert!(matches!(offset_error, AdminQueryError::BadRequest(_)));
        assert_eq!(
            offset_error.to_string(),
            "offset must be greater than or equal to 0"
        );
    }
}
