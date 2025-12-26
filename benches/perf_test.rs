mod common;

use common::{create_bench_database, BenchDatabase, BenchPayload};
use graphile_worker::{
    IntoTaskHandlerResult, JobSpec, LocalQueueConfig, RawJobSpec, RefetchDelayConfig, TaskHandler,
    WorkerContext,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

const JOB_COUNT: usize = 200_000;
const CONCURRENCY: usize = 96; // Node.js uses 4 workers × 24 = 96
const LOCAL_QUEUE_SIZE: usize = 500;
const REFETCH_DELAY_MS: u64 = 10;
const LATENCY_SAMPLES: usize = 1000;

#[derive(Clone, Debug)]
struct CompletedCounter(Arc<AtomicU64>);

#[derive(Clone, Debug)]
struct LatencySender(mpsc::Sender<Instant>);

impl TaskHandler for BenchPayload {
    const IDENTIFIER: &'static str = "log_if_999";

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
    const IDENTIFIER: &'static str = "latency";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        if let Some(sender) = ctx.get_ext::<LatencySender>() {
            let _ = sender.0.send(Instant::now()).await;
        }
        Ok::<(), String>(())
    }
}

fn create_worker_options_nodejs_style(db: &BenchDatabase) -> graphile_worker::WorkerOptions {
    db.create_worker_options()
        .concurrency(CONCURRENCY)
        .poll_interval(Duration::from_millis(100))
        .local_queue(
            LocalQueueConfig::default()
                .with_size(LOCAL_QUEUE_SIZE)
                .with_refetch_delay(
                    RefetchDelayConfig::builder()
                        .duration(Duration::from_millis(REFETCH_DELAY_MS))
                        .build(),
                ),
        )
        .complete_job_batch_delay(Duration::from_millis(5))
        .fail_job_batch_delay(Duration::from_millis(5))
}

async fn add_jobs_batch(db: &BenchDatabase, count: usize) {
    let utils = db.worker_utils();
    let batch_size = 10_000;

    for start in (0..count).step_by(batch_size) {
        let end = (start + batch_size).min(count);
        let jobs: Vec<RawJobSpec> = (start..end)
            .map(|i| RawJobSpec {
                identifier: "log_if_999".to_string(),
                payload: serde_json::to_value(BenchPayload::new(i as i32)).unwrap(),
                spec: JobSpec::default(),
            })
            .collect();

        utils.add_raw_jobs(&jobs).await.expect("Failed to add jobs");
    }
}

fn main() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        println!("=== Graphile Worker RS Performance Test ===");
        println!("Settings matching Node.js benchmark:");
        println!("  - Job count: {}", JOB_COUNT);
        println!("  - Concurrency: {}", CONCURRENCY);
        println!("  - Local queue size: {}", LOCAL_QUEUE_SIZE);
        println!("  - Refetch delay: {}ms", REFETCH_DELAY_MS);
        println!("  - Complete batch delay: 0ms");
        println!("  - Fail batch delay: 0ms");
        println!();

        let db = create_bench_database().await;

        let utils = db.worker_utils();
        utils.migrate().await.expect("Failed to migrate");

        println!("Timing startup/shutdown time...");
        let startup_start = Instant::now();
        {
            let _worker = create_worker_options_nodejs_style(&db)
                .define_job::<BenchPayload>()
                .init()
                .await
                .expect("Failed to init worker");
        }
        let startup_time = startup_start.elapsed();
        println!("... it took {}ms", startup_time.as_millis());
        println!();

        println!("Scheduling {} jobs...", JOB_COUNT);
        let add_start = Instant::now();
        add_jobs_batch(&db, JOB_COUNT).await;
        println!("... it took {}ms", add_start.elapsed().as_millis());
        println!();

        println!("Timing {} job execution...", JOB_COUNT);
        let completed = Arc::new(AtomicU64::new(0));
        let expected = JOB_COUNT as u64;

        let worker = Arc::new(
            create_worker_options_nodejs_style(&db)
                .add_extension(CompletedCounter(completed.clone()))
                .define_job::<BenchPayload>()
                .init()
                .await
                .expect("Failed to init worker"),
        );

        let worker_clone = Arc::clone(&worker);
        let worker_handle = tokio::spawn(async move {
            let _ = worker_clone.run().await;
        });

        let exec_start = Instant::now();

        while completed.load(Ordering::SeqCst) < expected {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let exec_time = exec_start.elapsed();
        worker.request_shutdown();
        let _ = worker_handle.await;

        let jobs_per_second = (JOB_COUNT as f64 / exec_time.as_secs_f64()).round();
        println!("... it took {}ms", exec_time.as_millis());
        println!("Jobs per second: {:.0}", jobs_per_second);
        println!();

        db.clear_jobs().await;

        println!("Testing latency ({} samples)...", LATENCY_SAMPLES);
        let (tx, mut rx) = mpsc::channel::<Instant>(100);

        let worker = Arc::new(
            create_worker_options_nodejs_style(&db)
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

        tokio::time::sleep(Duration::from_millis(500)).await;

        let mut latencies: Vec<Duration> = Vec::with_capacity(LATENCY_SAMPLES);

        for id in 0..LATENCY_SAMPLES {
            let start = Instant::now();
            utils
                .add_raw_job("latency", LatencyPayload { id: id as i32 }, JobSpec::default())
                .await
                .expect("Failed to add job");

            if let Some(end_time) = rx.recv().await {
                latencies.push(end_time.duration_since(start));
            }
        }

        worker.request_shutdown();
        let _ = worker_handle.await;

        let min = latencies.iter().min().unwrap();
        let max = latencies.iter().max().unwrap();
        let avg: Duration =
            latencies.iter().sum::<Duration>() / latencies.len() as u32;

        println!(
            "Latencies - min: {:.2}ms, max: {:.2}ms, avg: {:.2}ms",
            min.as_secs_f64() * 1000.0,
            max.as_secs_f64() * 1000.0,
            avg.as_secs_f64() * 1000.0
        );
        println!();

        db.drop().await;
        println!("Done");
    });
}
