use graphile_worker::sql::{
    batch_get_jobs::batch_get_jobs, return_jobs::batch::return_jobs,
    task_identifiers::get_tasks_details,
};
use graphile_worker::JobSpec;
use helpers::with_test_db;
use serde_json::json;

mod helpers;

// Run explicitly; database latency makes this unsuitable as a performance assertion.
/// Measures mixed-queue fetch/return cycles while checking claims and lock release.
///
/// This manual database workload has no performance threshold because latency
/// noise would make a speed assertion unreliable.
#[tokio::test]
#[ignore = "manual database fetch benchmark"]
async fn benchmark_cached_fetch_with_mixed_queues() {
    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        for i in 0..100 {
            db.add_job(
                "benchmark",
                json!({}),
                JobSpec {
                    queue_name: if i < 20 {
                        Some(format!("queue_{i}"))
                    } else {
                        None
                    },
                    run_at: Some(chrono::Utc::now() - chrono::Duration::minutes(1)),
                    ..Default::default()
                },
            )
            .await;
        }
        let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["benchmark".into()])
            .await
            .unwrap();
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let jobs = batch_get_jobs(
                &db.database,
                &tasks,
                "graphile_worker",
                "benchmark",
                &[],
                100,
                None,
            )
            .await
            .unwrap();
            assert_eq!(jobs.len(), 100);
            assert!(jobs.iter().all(|job| *job.attempts() == 1));
            return_jobs(&db.database, &jobs, "graphile_worker", "benchmark")
                .await
                .unwrap();
        }
        eprintln!(
            "1000 fetch/return cycles, 100 jobs (20 named queues + 80 unnamed): {:?}",
            start.elapsed()
        );
        assert!(db
            .get_jobs()
            .await
            .iter()
            .all(|job| job.locked_by.is_none() && job.attempts == 0));
    })
    .await;
}
