mod common;

use common::{create_bench_database, BenchDatabase, BenchPayload};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use graphile_worker::{JobSpec, RawJobSpec};
use tokio::runtime::Runtime;

async fn setup_worker(db: &BenchDatabase) {
    let _worker = db
        .create_worker_options()
        .init()
        .await
        .expect("Failed to init worker");
}

fn bench_add_raw_job(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db = rt.block_on(async {
        let db = create_bench_database().await;
        setup_worker(&db).await;
        db
    });

    c.bench_function("add_raw_job", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let db = db.clone();
            async move {
                let utils = db.worker_utils();
                let start = std::time::Instant::now();
                for i in 0..iters {
                    utils
                        .add_raw_job(
                            "bench_task",
                            BenchPayload::new(i as i32),
                            JobSpec::default(),
                        )
                        .await
                        .expect("Failed to add job");
                }
                db.clear_jobs().await;
                start.elapsed()
            }
        });
    });

    rt.block_on(db.drop());
}

fn bench_add_raw_jobs_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db = rt.block_on(async {
        let db = create_bench_database().await;
        setup_worker(&db).await;
        db
    });

    let mut group = c.benchmark_group("add_raw_jobs_batch");

    for batch_size in [10, 100, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &size| {
                b.to_async(&rt).iter_custom(|iters| {
                    let db = db.clone();
                    async move {
                        let utils = db.worker_utils();
                        let jobs: Vec<RawJobSpec> = (0..size)
                            .map(|i| RawJobSpec {
                                identifier: "bench_task".to_string(),
                                payload: serde_json::to_value(BenchPayload::new(i)).unwrap(),
                                spec: JobSpec::default(),
                            })
                            .collect();

                        let start = std::time::Instant::now();
                        for _ in 0..iters {
                            utils.add_raw_jobs(&jobs).await.expect("Failed to add jobs");
                        }
                        db.clear_jobs().await;
                        start.elapsed()
                    }
                });
            },
        );
    }
    group.finish();

    rt.block_on(db.drop());
}

fn bench_job_insertion_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let db = rt.block_on(async {
        let db = create_bench_database().await;
        setup_worker(&db).await;
        db
    });

    let mut group = c.benchmark_group("job_insertion_throughput");
    group.throughput(Throughput::Elements(100));
    group.sample_size(10);

    group.bench_function("100_jobs_sequential", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let db = db.clone();
            async move {
                let utils = db.worker_utils();
                let start = std::time::Instant::now();
                for _ in 0..iters {
                    for i in 0..100 {
                        utils
                            .add_raw_job("bench_task", BenchPayload::new(i), JobSpec::default())
                            .await
                            .expect("Failed to add job");
                    }
                }
                db.clear_jobs().await;
                start.elapsed()
            }
        });
    });

    group.bench_function("100_jobs_batched", |b| {
        b.to_async(&rt).iter_custom(|iters| {
            let db = db.clone();
            async move {
                let utils = db.worker_utils();
                let jobs: Vec<RawJobSpec> = (0..100)
                    .map(|i| RawJobSpec {
                        identifier: "bench_task".to_string(),
                        payload: serde_json::to_value(BenchPayload::new(i)).unwrap(),
                        spec: JobSpec::default(),
                    })
                    .collect();

                let start = std::time::Instant::now();
                for _ in 0..iters {
                    utils.add_raw_jobs(&jobs).await.expect("Failed to add jobs");
                }
                db.clear_jobs().await;
                start.elapsed()
            }
        });
    });

    group.finish();
    rt.block_on(db.drop());
}

criterion_group!(
    benches,
    bench_add_raw_job,
    bench_add_raw_jobs_batch,
    bench_job_insertion_throughput,
);
criterion_main!(benches);
