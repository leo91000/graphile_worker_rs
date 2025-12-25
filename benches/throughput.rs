mod common;

use common::{create_bench_database, BenchDatabase, BenchPayload};
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use graphile_worker::{IntoTaskHandlerResult, JobSpec, RawJobSpec, TaskHandler, WorkerContext};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
struct CompletedCounter(Arc<AtomicU64>);

#[derive(Clone, Debug)]
struct LatencySender(mpsc::Sender<std::time::Instant>);

impl TaskHandler for BenchPayload {
    const IDENTIFIER: &'static str = "noop_task";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        if let Some(counter) = ctx.get_ext::<CompletedCounter>() {
            counter.0.fetch_add(1, Ordering::SeqCst);
        }
        Ok::<(), String>(())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct LatencyPayload {
    id: i32,
}

impl TaskHandler for LatencyPayload {
    const IDENTIFIER: &'static str = "latency_task";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        if let Some(sender) = ctx.get_ext::<LatencySender>() {
            let _ = sender.0.send(std::time::Instant::now()).await;
        }
        Ok::<(), String>(())
    }
}

async fn setup_and_add_jobs(db: &BenchDatabase, count: usize) {
    let utils = db.worker_utils();
    let jobs: Vec<RawJobSpec> = (0..count)
        .map(|i| RawJobSpec {
            identifier: "noop_task".to_string(),
            payload: serde_json::to_value(BenchPayload::new(i as i32)).unwrap(),
            spec: JobSpec::default(),
        })
        .collect();

    utils.add_raw_jobs(&jobs).await.expect("Failed to add jobs");
}

fn bench_worker_startup_shutdown(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db = rt.block_on(create_bench_database());

    c.bench_function("worker_startup_shutdown", |b| {
        b.to_async(&rt).iter(|| {
            let db = db.clone();
            async move {
                let _worker = db
                    .create_worker_options()
                    .define_job::<BenchPayload>()
                    .init()
                    .await
                    .expect("Failed to init worker");
            }
        });
    });

    rt.block_on(db.drop());
}

fn bench_job_execution_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db = rt.block_on(create_bench_database());

    let mut group = c.benchmark_group("job_execution");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    for job_count in [100, 1000] {
        group.throughput(Throughput::Elements(job_count as u64));

        group.bench_function(format!("{}_jobs", job_count), |b| {
            b.to_async(&rt).iter_custom(|iters| {
                let db = db.clone();
                async move {
                    let mut total_elapsed = Duration::ZERO;

                    for _ in 0..iters {
                        let completed = Arc::new(AtomicU64::new(0));
                        let expected = job_count as u64;

                        let worker = Arc::new(
                            db.create_worker_options()
                                .concurrency(8)
                                .add_extension(CompletedCounter(completed.clone()))
                                .define_job::<BenchPayload>()
                                .init()
                                .await
                                .expect("Failed to init worker"),
                        );

                        setup_and_add_jobs(&db, job_count).await;

                        let worker_clone = Arc::clone(&worker);
                        let worker_handle = tokio::spawn(async move {
                            let _ = worker_clone.run().await;
                        });

                        let start = std::time::Instant::now();

                        while completed.load(Ordering::SeqCst) < expected {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }

                        total_elapsed += start.elapsed();

                        worker.request_shutdown();
                        let _ = worker_handle.await;
                        db.clear_jobs().await;
                    }

                    total_elapsed
                }
            });
        });
    }

    group.finish();
    rt.block_on(db.drop());
}

fn bench_job_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db = rt.block_on(create_bench_database());

    let mut group = c.benchmark_group("job_latency");
    group.sample_size(50);

    group.bench_function("single_job_latency", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let db = db.clone();
            async move {
                let mut total_elapsed = Duration::ZERO;

                let (tx, mut rx) = mpsc::channel::<std::time::Instant>(1);

                let worker = Arc::new(
                    db.create_worker_options()
                        .concurrency(1)
                        .add_extension(LatencySender(tx))
                        .define_job::<LatencyPayload>()
                        .init()
                        .await
                        .expect("Failed to init worker"),
                );

                let worker_clone = Arc::clone(&worker);
                let worker_handle = tokio::spawn(async move {
                    let _ = worker_clone.run().await;
                });

                for i in 0..iters {
                    let utils = db.worker_utils();
                    let start = std::time::Instant::now();
                    utils
                        .add_raw_job(
                            "latency_task",
                            LatencyPayload { id: i as i32 },
                            JobSpec::default(),
                        )
                        .await
                        .expect("Failed to add job");

                    if let Some(end_time) = rx.recv().await {
                        total_elapsed += end_time.duration_since(start);
                    }
                }

                worker.request_shutdown();
                let _ = worker_handle.await;
                db.clear_jobs().await;

                total_elapsed
            }
        });
    });

    group.finish();
    rt.block_on(db.drop());
}

criterion_group!(
    benches,
    bench_worker_startup_shutdown,
    bench_job_execution_throughput,
    bench_job_latency,
);
criterion_main!(benches);
