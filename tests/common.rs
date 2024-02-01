use chrono::{DateTime, Utc};
use graphile_worker::WorkerOptions;
use serde_json::Value;
use sqlx::postgres::PgConnectOptions;
use sqlx::FromRow;
use sqlx::PgPool;
use tokio::task::LocalSet;

#[derive(FromRow, Debug)]
#[allow(dead_code)]
pub struct Job {
    pub id: i64,
    pub job_queue_id: Option<i32>,
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
}

#[derive(Clone)]
pub struct TestDatabase {
    pub source_pool: PgPool,
    pub test_pool: PgPool,
    pub name: String,
}

impl TestDatabase {
    async fn drop(&self) {
        self.test_pool.close().await;
        sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.name))
            .execute(&self.source_pool)
            .await
            .expect("Failed to drop test database");
    }

    pub fn create_worker_options(&self) -> WorkerOptions {
        WorkerOptions::default()
            .pg_pool(self.test_pool.clone())
            .schema("graphile_worker")
            .concurrency(4)
    }

    pub async fn get_jobs(&self) -> Vec<Job> {
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
}

async fn create_test_database() -> TestDatabase {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pg_conn_options: PgConnectOptions = db_url.parse().expect("Failed to parse DATABASE_URL");

    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(pg_conn_options.clone())
        .await
        .expect("Failed to connect to database");

    let db_id = uuid::Uuid::now_v7();
    let db_name = format!("__test_graphile_worker_{}", db_id.simple());

    sqlx::query(&format!("CREATE DATABASE {}", db_name))
        .execute(&pg_pool)
        .await
        .expect("Failed to create test database");

    sqlx::query(&format!("CREATE SCHEMA {}", db_name))
        .execute(&pg_pool)
        .await
        .expect("Failed to create test schema");

    let test_options = pg_conn_options.database(&db_name);

    let test_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(test_options)
        .await
        .expect("Failed to connect to test database");

    TestDatabase {
        source_pool: pg_pool,
        test_pool,
        name: db_name,
    }
}

// The `with_test_db` function
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
