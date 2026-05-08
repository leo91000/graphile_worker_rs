#![allow(dead_code)]

use chrono::{DateTime, Local, Utc};
use graphile_worker::sql::add_job::add_job;
use graphile_worker::Database;
use graphile_worker::DbJob;
use graphile_worker::Job;
use graphile_worker::JobSpec;
use graphile_worker::WorkerOptions;
use graphile_worker::WorkerUtils;
use graphile_worker_crontab_runner::KnownCrontab;
use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgRow};
use sqlx::FromRow;
use sqlx::PgPool;
use sqlx::Row;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tokio::task::LocalSet;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

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

#[derive(Clone)]
pub struct SelectionOfJobs {
    pub failed_job: Job,
    pub regular_job_1: Job,
    pub locked_job: Job,
    pub regular_job_2: Job,
    pub untouched_job: Job,
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

    pub async fn make_selection_of_jobs(&self) -> SelectionOfJobs {
        let utils = self.worker_utils();

        let in_one_hour = Utc::now() + chrono::Duration::hours(1);
        let failed_job = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 1 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");
        let regular_job_1 = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 2 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");
        let locked_job = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 3 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");
        let regular_job_2 = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 4 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");
        let untouched_job = utils
            .add_raw_job(
                "job3",
                serde_json::json!({ "a": 5 }),
                JobSpec {
                    run_at: Some(in_one_hour),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add job");

        let locked_job = {
            let locked_job_update = sqlx::query("update graphile_worker._private_jobs as jobs set locked_by = 'test', locked_at = now() where id = $1 returning *")
            .bind(locked_job.id())
            .fetch_one(&self.test_pool)
            .await
            .expect("Failed to lock job");

            Job::from_db_job(db_job_from_sqlx_row(locked_job_update), "job3".to_string())
        };

        let failed_job = {
            let failed_job_update = sqlx::query("update graphile_worker._private_jobs as jobs set last_error = 'Failed forever', attempts = max_attempts where id = $1 returning *")
                .bind(failed_job.id())
                .fetch_one(&self.test_pool)
                .await
                .expect("Failed to fail job");

            Job::from_db_job(db_job_from_sqlx_row(failed_job_update), "job3".to_string())
        };

        SelectionOfJobs {
            failed_job,
            regular_job_1,
            locked_job,
            regular_job_2,
            untouched_job,
        }
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

fn db_job_from_sqlx_row(row: PgRow) -> DbJob {
    DbJob::new(
        row.get("id"),
        row.get("job_queue_id"),
        row.get("payload"),
        row.get("priority"),
        row.get("run_at"),
        row.get("attempts"),
        row.get("max_attempts"),
        row.get("last_error"),
        row.get("created_at"),
        row.get("updated_at"),
        row.get("key"),
        row.get("revision"),
        row.get("locked_at"),
        row.get("locked_by"),
        row.get("flags"),
        row.get("task_id"),
    )
}

fn known_crontab_from_sqlx_row(row: PgRow) -> KnownCrontab {
    let known_since: DateTime<Utc> = row.get("known_since");
    let last_execution: Option<DateTime<Utc>> = row.get("last_execution");

    KnownCrontab::new(
        row.get("identifier"),
        known_since.with_timezone(&Local),
        last_execution.map(|value| value.with_timezone(&Local)),
    )
}

fn test_database_url(db_url: &str, db_name: &str) -> String {
    let (base_url, query_string) = db_url
        .split_once('?')
        .map(|(base_url, query_string)| (base_url, Some(query_string)))
        .unwrap_or((db_url, None));
    let Some((server_url, _database_name)) = base_url.rsplit_once('/') else {
        return db_url.to_string();
    };

    match query_string {
        Some(query_string) => format!("{server_url}/{db_name}?{query_string}"),
        None => format!("{server_url}/{db_name}"),
    }
}

fn create_graphile_database(test_pool: PgPool, test_database_url: &str) -> Database {
    #[cfg(feature = "driver-tokio-postgres")]
    {
        let _ = test_pool;
        return graphile_worker::tokio_postgres::TokioPostgresDatabase::from_url(
            test_database_url,
            2,
        )
        .expect("Failed to create tokio-postgres database")
        .into();
    }

    #[cfg(all(not(feature = "driver-tokio-postgres"), feature = "driver-sqlx"))]
    {
        let _ = test_database_url;
        test_pool.into()
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

    sqlx::query(&format!("CREATE DATABASE {}", db_name))
        .execute(&pg_pool)
        .await
        .expect("Failed to create test database");

    sqlx::query(&format!("CREATE SCHEMA {}", db_name))
        .execute(&pg_pool)
        .await
        .expect("Failed to create test schema");

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

pub struct StaticCounter {
    cell: OnceCell<Mutex<u32>>,
}
async fn init_job_count() -> Mutex<u32> {
    Mutex::new(0)
}
impl StaticCounter {
    pub const fn new() -> Self {
        Self {
            cell: OnceCell::const_new(),
        }
    }

    pub async fn increment(&self) -> u32 {
        let cell = self.cell.get_or_init(init_job_count).await;
        let mut count = cell.lock().await;
        *count += 1;
        *count
    }

    pub async fn get(&self) -> u32 {
        let cell = self.cell.get_or_init(init_job_count).await;
        *cell.lock().await
    }

    pub async fn reset(&self) {
        let cell = self.cell.get_or_init(init_job_count).await;
        let mut count = cell.lock().await;
        *count = 0;
    }
}

impl Default for StaticCounter {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn enable_logs() {
    static ONCE: OnceCell<()> = OnceCell::const_new();

    ONCE.get_or_init(|| async {
        let fmt_layer = tracing_subscriber::fmt::layer();
        // Log level set to debug except for sqlx set at warn (to not show all sql requests)
        let filter_layer = EnvFilter::try_new("debug,sqlx=warn").unwrap();

        tracing_subscriber::registry()
            .with(filter_layer)
            .with(fmt_layer)
            .init();
    })
    .await;
}
