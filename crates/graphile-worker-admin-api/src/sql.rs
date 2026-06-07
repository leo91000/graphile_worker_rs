use graphile_worker_database::Schema;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::{FromRow, Postgres};

#[derive(Debug, Clone, Copy)]
pub(crate) enum AdminTable {
    Jobs,
    JobQueues,
    Tasks,
}

impl AdminTable {
    pub(crate) fn qualified(self, schema: &Schema) -> String {
        schema.private_table(self.as_str())
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Jobs => "jobs",
            Self::JobQueues => "job_queues",
            Self::Tasks => "tasks",
        }
    }
}

pub(crate) fn safe_query_as<T>(sql: &str) -> sqlx::query::QueryAs<'_, Postgres, T, PgArguments>
where
    T: for<'row> FromRow<'row, PgRow> + Send + Unpin,
{
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
}
