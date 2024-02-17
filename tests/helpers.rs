#![allow(dead_code)]

use chrono::{DateTime, Utc};
use graphile_worker::sql::add_job::add_job;
use graphile_worker::DbJob;
use graphile_worker::Job;
use graphile_worker::JobSpec;
use graphile_worker::WorkerOptions;
use graphile_worker::WorkerUtils;
use graphile_worker_crontab_runner::KnownCrontab;
use serde_json::Value;
use sqlx::postgres::PgConnectOptions;
use sqlx::query_as;
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
            .pg_pool(self.test_pool.clone())
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
            &self.test_pool,
            "graphile_worker",
            identifier,
            serde_json::to_value(payload).unwrap(),
            spec,
        )
        .await
        .expect("Failed to add job")
    }

    pub fn worker_utils(&self) -> WorkerUtils {
        WorkerUtils::new(self.test_pool.clone(), "graphile_worker".into())
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
            let locked_job_update: DbJob = query_as("update graphile_worker._private_jobs as jobs set locked_by = 'test', locked_at = now() where id = $1 returning *")
            .bind(locked_job.id())
            .fetch_one(&self.test_pool)
            .await
            .expect("Failed to lock job");

            Job::from_db_job(locked_job_update, "job3".to_string())
        };

        let failed_job = {
            let failed_job_update: DbJob = query_as("update graphile_worker._private_jobs as jobs set last_error = 'Failed forever', attempts = max_attempts where id = $1 returning *")
                .bind(failed_job.id())
                .fetch_one(&self.test_pool)
                .await
                .expect("Failed to fail job");

            Job::from_db_job(failed_job_update, "job3".to_string())
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
        sqlx::query_as("select * from graphile_worker._private_known_crontabs")
            .fetch_all(&self.test_pool)
            .await
            .expect("Failed to get known crontabs")
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

    let test_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(test_options)
        .await
        .expect("Failed to connect to test database");

    TestDatabase {
        source_pool: pg_pool,
        test_pool,
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
