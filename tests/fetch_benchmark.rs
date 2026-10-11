use graphile_worker::sql::{
    batch_get_jobs::batch_get_jobs, return_jobs::batch::return_jobs,
    task_identifiers::get_tasks_details,
};
use graphile_worker::JobSpec;
use helpers::with_test_db;
use serde_json::json;

mod helpers;

/// Measures real ordered claims with equivalent accepted, forbidden and
/// allowlist policies. Fixtures and returned ownership stay isolated per run.
#[tokio::test]
#[ignore = "manual allowlist claim benchmark"]
async fn benchmark_job_flag_allowlist() {
    use graphile_worker::sql::{
        batch_get_jobs::batch_get_jobs_with_filter, get_job::get_job_with_filter,
    };
    use graphile_worker::JobFlagFilter;
    use std::time::Instant;

    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["benchmark".into()]).await.unwrap();
        let task_id = tasks.task_ids()[0];
        let version: String = helpers::sql::safe_query_scalar("SELECT version()").fetch_one(&db.test_pool).await.unwrap();
        eprintln!("ALLOWLIST_BENCH_SERVER {version}");
        let rows = std::env::var("ALLOWLIST_BENCH_ROWS").ok().map(|value| value.parse::<i32>().unwrap()).unwrap_or(100_000);
        for (flag_count, allowed_count) in [(1, 1), (4, 8), (16, 64)] {
            let allowed = std::iter::once("eligible".to_owned()).chain((1..allowed_count).map(|index| format!("flag-{index}"))).collect::<Vec<_>>();
            let common = (1..flag_count).map(|index| format!("flag-{index}")).collect::<Vec<_>>();
            for rejection in [0.1, 0.9, 0.999] {
                helpers::sql::safe_query("TRUNCATE graphile_worker._private_jobs, graphile_worker._private_job_retirements").execute(&db.test_pool).await.unwrap();
                let rejected = (f64::from(rows) * rejection) as i32;
                helpers::sql::safe_query("INSERT INTO graphile_worker._private_jobs(task_id,payload,run_at,flags) SELECT $1, json_build_object('index',n), timestamp '2020-01-01' + n * interval '1 microsecond', (SELECT jsonb_object_agg(flag,true) FROM unnest($4::text[] || ARRAY[CASE WHEN n <= $3 THEN 'unsupported' ELSE 'eligible' END]) AS flag) FROM generate_series(1,$2) AS n")
                    .bind(task_id).bind(rows).bind(rejected).bind(&common).execute(&db.test_pool).await.unwrap();
                helpers::sql::safe_query("ANALYZE graphile_worker._private_jobs").execute(&db.test_pool).await.unwrap();
                let filters = [
                    ("accepted", JobFlagFilter::owned(vec![], vec!["eligible".into()])),
                    ("forbidden", JobFlagFilter::owned(vec!["unsupported".into()], vec![])),
                    ("allowlist", JobFlagFilter::default().with_allowlist(allowed.clone())),
                ];
                for size in [1, 10] {
                    let mut samples: [Vec<f64>;3] = std::array::from_fn(|_| Vec::new());
                    for round in 0..15 {
                        for offset in 0..filters.len() {
                            let index = (round + offset) % filters.len();
                            let (name, filter) = &filters[index];
                            let start = Instant::now();
                            let jobs = if size == 1 {
                                get_job_with_filter(&db.database, &tasks, "graphile_worker", "benchmark", filter.as_borrowed(), None).await.unwrap().into_iter().collect::<Vec<_>>()
                            } else {
                                batch_get_jobs_with_filter(&db.database, &tasks, "graphile_worker", "benchmark", filter.as_borrowed(), size, None).await.unwrap()
                            };
                            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                            assert_eq!(jobs.len(), size as usize, "{name}");
                            assert!(jobs.iter().all(|job| job.payload()["index"].as_i64().unwrap() > i64::from(rejected)));
                            if round >= 3 { samples[index].push(elapsed); }
                            return_jobs(&db.database, &jobs, "graphile_worker", "benchmark").await.unwrap();
                        }
                    }
                    for (index, (name, _)) in filters.iter().enumerate() {
                        let values = &mut samples[index];
                        values.sort_by(f64::total_cmp);
                        eprintln!("ALLOWLIST_BENCH rows={rows} rejection={rejection} flags={flag_count} allowed={allowed_count} batch={size} filter={name} median_ms={:.3} p95_ms={:.3}", values[values.len()/2], values[(values.len()*95/100).min(values.len()-1)]);
                    }
                }
            }
        }
    }).await;
}

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
