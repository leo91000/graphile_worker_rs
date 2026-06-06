use chrono::{DateTime, Utc};
use graphile_worker::sql::add_job::add_job;
use graphile_worker::{Database, Job, JobSpec, WorkerOptions, WorkerUtils};
use graphile_worker_crontab_runner::KnownCrontab;
use serde_json::Value;
use sqlx::postgres::PgConnectOptions;
use sqlx::{FromRow, PgPool, Row};
use tokio::task::LocalSet;

use super::sql::{known_crontab_from_sqlx_row, safe_query};

#[derive(FromRow, Debug, PartialEq, Eq)]
pub struct JobWithQueueName {
    pub id: i64,
    pub job_queue_id: Option<i32>,
    pub task_identifier: String,
    pub queue_name: Option<String>,
    pub payload: serde_json::Value,
    pub priority: i16,
    pub run_at: DateTime<Utc>,
    pub attempts: i16,
    pub max_attempts: i16,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub key: Option<String>,
    pub revision: i32,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub flags: Option<Value>,
    pub task_id: i32,
    pub is_available: bool,
}

#[derive(FromRow, Debug)]
pub struct JobQueue {
    pub id: i32,
    pub queue_name: String,
    pub job_count: i32,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
}

#[derive(FromRow, Debug)]
pub struct Migration {
    pub id: i32,
    pub ts: DateTime<Utc>,
    pub breaking: bool,
}

#[derive(Clone, Debug)]
pub struct TestDatabase {
    pub source_pool: PgPool,
    pub test_pool: PgPool,
    pub database: Database,
    pub name: String,
}

impl TestDatabase {
    async fn drop(&self) {
        self.test_pool.close().await;
        safe_query(format!("DROP DATABASE {} WITH (FORCE)", self.name))
            .execute(&self.source_pool)
            .await
            .expect("Failed to drop test database");
    }

    pub fn create_worker_options(&self) -> WorkerOptions {
        WorkerOptions::default()
            .database(self.database.clone())
            .schema("graphile_worker")
            .concurrency(4)
    }

    pub async fn get_jobs(&self) -> Vec<JobWithQueueName> {
        sqlx::query_as(
            r#"
                select jobs.*, identifier as task_identifier, job_queues.queue_name as queue_name
                    from graphile_worker._private_jobs as jobs
                    left join graphile_worker._private_tasks as tasks on jobs.task_id = tasks.id
                    left join graphile_worker._private_job_queues as job_queues on jobs.job_queue_id = job_queues.id
                    order by jobs.id asc
            "#
            )
            .fetch_all(&self.test_pool)
            .await
            .expect("Failed to get jobs")
    }

    pub async fn get_job_queues(&self) -> Vec<JobQueue> {
        sqlx::query_as(
            r#"
                select job_queues.*, count(jobs.*)::int as job_count
                    from graphile_worker._private_job_queues as job_queues
                    left join graphile_worker._private_jobs as jobs on (
                        jobs.job_queue_id = job_queues.id
                    )
                    group by job_queues.id
                    order by job_queues.queue_name asc
            "#,
        )
        .fetch_all(&self.test_pool)
        .await
        .expect("Failed to get job queues")
    }

    pub async fn get_tasks(&self) -> Vec<String> {
        let rows = sqlx::query("select identifier from graphile_worker._private_tasks")
            .fetch_all(&self.test_pool)
            .await
            .expect("Failed to get tasks");

        rows.into_iter().map(|row| row.get("identifier")).collect()
    }

    pub async fn get_task_id(&self, identifier: &str) -> i32 {
        let row =
            sqlx::query("select id from graphile_worker._private_tasks where identifier = $1")
                .bind(identifier)
                .fetch_one(&self.test_pool)
                .await
                .expect("Failed to get task id");

        row.get("id")
    }

    pub async fn make_jobs_run_now(&self, task_identifier: &str) {
        sqlx::query(
            r#"
                update graphile_worker._private_jobs
                    set run_at = now()
                    where task_id = (
                        select id
                            from graphile_worker._private_tasks
                            where identifier = $1
                    )
            "#,
        )
        .bind(task_identifier)
        .execute(&self.test_pool)
        .await
        .expect("Failed to update jobs");
    }

    pub async fn get_migrations(&self) -> Vec<Migration> {
        sqlx::query_as("select * from graphile_worker.migrations")
            .fetch_all(&self.test_pool)
            .await
            .expect("Failed to get migrations")
    }

    pub async fn add_job(
        &self,
        identifier: &str,
        payload: impl serde::Serialize,
        spec: JobSpec,
    ) -> Job {
        add_job(
            &self.database,
            "graphile_worker",
            identifier,
            serde_json::to_value(payload).unwrap(),
            spec,
            true,
        )
        .await
        .expect("Failed to add job")
    }

    pub fn worker_utils(&self) -> WorkerUtils {
        WorkerUtils::new(self.database.clone(), "graphile_worker".into())
    }

    pub async fn get_known_crontabs(&self) -> Vec<KnownCrontab> {
        sqlx::query("select * from graphile_worker._private_known_crontabs")
            .fetch_all(&self.test_pool)
            .await
            .expect("Failed to get known crontabs")
            .into_iter()
            .map(known_crontab_from_sqlx_row)
            .collect()
    }
}

fn test_database_url(db_url: &str, db_name: &str) -> String {
    let Ok(mut url) = url::Url::parse(db_url) else {
        return db_url.to_string();
    };

    url.set_path(db_name);
    url.to_string()
}

fn create_graphile_database(test_pool: PgPool, test_database_url: &str) -> Database {
    #[cfg(feature = "driver-tokio-postgres")]
    {
        let _ = test_pool;
        graphile_worker::tokio_postgres::TokioPostgresDatabase::from_url(test_database_url, 2)
            .expect("Failed to create tokio-postgres database")
            .into()
    }

    #[cfg(all(not(feature = "driver-tokio-postgres"), feature = "driver-sqlx"))]
    {
        let _ = test_database_url;
        test_pool.into()
    }

    #[cfg(all(not(feature = "driver-tokio-postgres"), not(feature = "driver-sqlx")))]
    {
        let _ = (test_pool, test_database_url);
        compile_error!("create_graphile_database requires a PostgreSQL driver feature");
    }
}

pub async fn create_test_database() -> TestDatabase {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut pg_conn_options: PgConnectOptions =
        db_url.parse().expect("Failed to parse DATABASE_URL");
    pg_conn_options = pg_conn_options.application_name("__test_graphile_worker");

    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(pg_conn_options.clone())
        .await
        .expect("Failed to connect to database");

    let db_id = uuid::Uuid::now_v7();
    let db_name = format!("__test_graphile_worker_{}", db_id.simple());

    safe_query(format!("CREATE DATABASE {}", db_name))
        .execute(&pg_pool)
        .await
        .expect("Failed to create test database");

    let test_options = pg_conn_options.database(&db_name);
    let test_database_url = test_database_url(&db_url, &db_name);

    let test_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(test_options)
        .await
        .expect("Failed to connect to test database");
    let database = create_graphile_database(test_pool.clone(), &test_database_url);

    TestDatabase {
        source_pool: pg_pool,
        test_pool,
        database,
        name: db_name,
    }
}

pub async fn with_test_db<F, Fut>(test_fn: F)
where
    F: FnOnce(TestDatabase) -> Fut + 'static,
    Fut: std::future::Future<Output = ()>,
{
    let local_set = LocalSet::new();

    local_set
        .run_until(async move {
            let test_db = create_test_database().await;
            let test_db_2 = test_db.clone();

            let result = tokio::task::spawn_local(async move {
                test_fn(test_db_2).await;
            })
            .await;

            test_db.drop().await;
            result.expect("Test failed");
        })
        .await;
}
