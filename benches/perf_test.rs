mod common;

use common::{create_bench_database, BenchPayload};
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, LocalQueueConfig, RawJobSpec, TaskHandler, WorkerContext,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

const JOB_COUNT: usize = 200_000;

#[derive(Clone, Debug)]
struct CompletedCounter(Arc<AtomicU64>);

impl TaskHandler for BenchPayload {
    const IDENTIFIER: &'static str = "bench_task";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        if let Some(counter) = ctx.get_ext::<CompletedCounter>() {
            counter.0.fetch_add(1, Ordering::Relaxed);
        }
        Ok::<(), String>(())
    }
}

async fn run_benchmark(with_local_queue: bool) -> Duration {
    let db = create_bench_database().await;
    let utils = db.worker_utils();
    utils.migrate().await.expect("Failed to migrate");

    let jobs: Vec<RawJobSpec> = (0..JOB_COUNT)
        .map(|i| RawJobSpec {
            identifier: "bench_task".to_string(),
            payload: serde_json::json!({"id": i, "data": format!("test_{}", i)}),
            spec: JobSpec::default(),
        })
        .collect();
    utils.add_raw_jobs(&jobs).await.unwrap();

    let completed = Arc::new(AtomicU64::new(0));

    let mut options = db
        .create_worker_options()
        .concurrency(500)
        .poll_interval(Duration::from_millis(5))
        .complete_job_batch_delay(Duration::from_millis(5))
        .add_extension(CompletedCounter(completed.clone()))
        .define_job::<BenchPayload>();

    if with_local_queue {
        options = options.local_queue(LocalQueueConfig::default().with_size(5000));
    }

    let worker = Arc::new(options.init().await.unwrap());

    let worker_clone = Arc::clone(&worker);
    let handle = tokio::spawn(async move { worker_clone.run().await });

    let start = Instant::now();
    while completed.load(Ordering::Relaxed) < JOB_COUNT as u64 {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let elapsed = start.elapsed();

    worker.request_shutdown();
    handle
        .await
        .expect("Worker task panicked")
        .expect("Worker failed");

    let jobs_per_sec = JOB_COUNT as f64 / elapsed.as_secs_f64();
    println!(
        "Completed {} jobs in {:?} ({:.0} jobs/sec)",
        JOB_COUNT, elapsed, jobs_per_sec
    );

    db.drop().await;
    elapsed
}

fn bench_local_queue_execution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("perf_test");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.throughput(Throughput::Elements(JOB_COUNT as u64));

    group.bench_function("local_queue_200k_execution", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let mut total_elapsed = Duration::ZERO;
            for _ in 0..iters {
                total_elapsed += run_benchmark(true).await;
            }
            total_elapsed
        });
    });

    group.finish();
}

criterion_group!(benches, bench_local_queue_execution);
criterion_main!(benches);
