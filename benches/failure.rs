mod common;

use common::{create_bench_database, BenchDatabase};
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, LocalQueueConfig, RawJobSpec, TaskHandler, WorkerContext,
};
use indoc::indoc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

const JOB_COUNT: usize = 1_000;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct RetryFailurePayload {
    id: i32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct PermanentFailurePayload {
    id: i32,
}

impl TaskHandler for RetryFailurePayload {
    const IDENTIFIER: &'static str = "retry_failure_task";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Err::<(), String>(format!("retry failure {}", self.id))
    }
}

impl TaskHandler for PermanentFailurePayload {
    const IDENTIFIER: &'static str = "permanent_failure_task";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        Err::<(), String>(format!("permanent failure {}", self.id))
    }
}

enum FailureKind {
    Retry,
    Permanent,
}

impl FailureKind {
    fn identifier(&self) -> &'static str {
        match self {
            Self::Retry => RetryFailurePayload::IDENTIFIER,
            Self::Permanent => PermanentFailurePayload::IDENTIFIER,
        }
    }

    fn job_spec(&self) -> JobSpec {
        match self {
            Self::Retry => JobSpec::builder().max_attempts(3).build(),
            Self::Permanent => JobSpec::builder().max_attempts(1).build(),
        }
    }
}

async fn setup_failed_jobs(db: &BenchDatabase, kind: &FailureKind) {
    let utils = db.worker_utils();
    utils.migrate().await.expect("Failed to migrate");

    let identifier = kind.identifier();
    let spec = kind.job_spec();
    let jobs: Vec<RawJobSpec> = (0..JOB_COUNT)
        .map(|i| RawJobSpec {
            identifier: identifier.to_string(),
            payload: serde_json::json!({ "id": i }),
            spec: spec.clone(),
        })
        .collect();

    utils.add_raw_jobs(&jobs).await.expect("Failed to add jobs");
}

async fn failed_unlocked_count(db: &BenchDatabase) -> i64 {
    let row: (i64,) = sqlx::query_as(indoc! {r#"
            SELECT COUNT(*)
            FROM graphile_worker._private_jobs
            WHERE locked_by IS NULL
                AND last_error IS NOT NULL
                AND attempts = 1
        "#})
    .fetch_one(&db.bench_pool)
    .await
    .expect("Failed to count failed jobs");

    row.0
}

async fn wait_for_failed_jobs(db: &BenchDatabase) {
    let expected = JOB_COUNT as i64;
    let wait = async {
        loop {
            if failed_unlocked_count(db).await >= expected {
                return;
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };

    tokio::time::timeout(Duration::from_secs(60), wait)
        .await
        .expect("Timed out waiting for failed jobs");
}

async fn run_failure_benchmark(kind: FailureKind) -> Duration {
    let db = create_bench_database().await;
    setup_failed_jobs(&db, &kind).await;

    let worker = Arc::new(
        db.create_worker_options()
            .concurrency(500)
            .poll_interval(Duration::from_millis(5))
            .fail_job_batch_delay(Duration::from_millis(5))
            .local_queue(LocalQueueConfig::default().with_size(5000))
            .define_job::<RetryFailurePayload>()
            .define_job::<PermanentFailurePayload>()
            .init()
            .await
            .expect("Failed to init worker"),
    );

    let worker_clone = Arc::clone(&worker);
    let handle = tokio::spawn(async move { worker_clone.run().await });

    let start = Instant::now();
    wait_for_failed_jobs(&db).await;
    let elapsed = start.elapsed();

    worker.request_shutdown();
    handle
        .await
        .expect("Worker task panicked")
        .expect("Worker failed");

    db.drop().await;
    elapsed
}

fn bench_failure_handling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("failure_handling");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.throughput(Throughput::Elements(JOB_COUNT as u64));

    group.bench_function(format!("retry_{JOB_COUNT}_jobs"), |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let mut total_elapsed = Duration::ZERO;
            for _ in 0..iters {
                total_elapsed += run_failure_benchmark(FailureKind::Retry).await;
            }
            total_elapsed
        });
    });

    group.bench_function(format!("permanent_{JOB_COUNT}_jobs"), |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let mut total_elapsed = Duration::ZERO;
            for _ in 0..iters {
                total_elapsed += run_failure_benchmark(FailureKind::Permanent).await;
            }
            total_elapsed
        });
    });

    group.finish();
}

criterion_group!(benches, bench_failure_handling);
criterion_main!(benches);
