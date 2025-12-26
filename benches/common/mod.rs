#![allow(dead_code)]

use graphile_worker::{LocalQueueConfig, WorkerOptions, WorkerUtils};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct BenchDatabase {
    pub source_pool: PgPool,
    pub bench_pool: PgPool,
    pub name: String,
}

impl BenchDatabase {
    pub async fn drop(&self) {
        self.bench_pool.close().await;
        sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.name))
            .execute(&self.source_pool)
            .await
            .expect("Failed to drop bench database");
    }

    pub fn create_worker_options(&self) -> WorkerOptions {
        WorkerOptions::default()
            .pg_pool(self.bench_pool.clone())
            .schema("graphile_worker")
            .concurrency(4)
            .poll_interval(Duration::from_millis(10))
            .local_queue(LocalQueueConfig::default().with_size(100))
    }

    pub fn worker_utils(&self) -> WorkerUtils {
        WorkerUtils::new(self.bench_pool.clone(), "graphile_worker".into())
    }

    pub async fn clear_jobs(&self) {
        sqlx::query("DELETE FROM graphile_worker._private_jobs")
            .execute(&self.bench_pool)
            .await
            .expect("Failed to clear jobs");
    }

    pub async fn job_count(&self) -> i64 {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM graphile_worker._private_jobs")
            .fetch_one(&self.bench_pool)
            .await
            .expect("Failed to count jobs");
        row.0
    }
}

pub async fn create_bench_database() -> BenchDatabase {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut pg_conn_options: PgConnectOptions =
        db_url.parse().expect("Failed to parse DATABASE_URL");
    pg_conn_options = pg_conn_options.application_name("__bench_graphile_worker");

    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(pg_conn_options.clone())
        .await
        .expect("Failed to connect to database");

    let db_id = Uuid::now_v7();
    let db_name = format!("__bench_graphile_worker_{}", db_id.simple());

    sqlx::query(&format!("CREATE DATABASE {}", db_name))
        .execute(&pg_pool)
        .await
        .expect("Failed to create bench database");

    let bench_options = pg_conn_options.database(&db_name);

    let bench_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(100) // Support high concurrency benchmarks
        .connect_with(bench_options)
        .await
        .expect("Failed to connect to bench database");

    BenchDatabase {
        source_pool: pg_pool,
        bench_pool,
        name: db_name,
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BenchPayload {
    pub id: i32,
    pub data: String,
}

impl BenchPayload {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            data: format!("benchmark_payload_{}", id),
        }
    }
}
