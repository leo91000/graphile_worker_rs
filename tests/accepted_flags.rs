use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use graphile_worker::sql::{
    batch_get_jobs::batch_get_jobs_with_filter, get_job::get_job_with_filter,
    return_jobs::batch::return_jobs, task_identifiers::get_tasks_details,
};
use graphile_worker::{
    IntoTaskHandlerResult, JobFlagFilter, JobSpec, LocalQueueConfig, LocalQueueInit, Schema,
    TaskHandler, Worker, WorkerContext, WorkerUtils,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::{sleep, timeout};

mod helpers;
use helpers::with_test_db;

#[derive(Clone, Debug, Default)]
struct Executions(Arc<Mutex<Vec<u32>>>);

#[derive(Serialize, Deserialize)]
struct FlaggedJob {
    value: u32,
}

impl TaskHandler for FlaggedJob {
    const IDENTIFIER: &'static str = "accepted_flags_job";

    async fn run(self, ctx: WorkerContext) -> impl IntoTaskHandlerResult {
        ctx.get_ext::<Executions>()
            .expect("execution recorder")
            .0
            .lock()
            .unwrap()
            .push(self.value);
    }
}

async fn worker_routes_jobs(
    accepted: Vec<&'static str>,
    forbidden: Vec<&'static str>,
    expected: Vec<u32>,
    continuous: bool,
) {
    with_test_db(move |db| async move {
        let executions = Executions::default();
        let local_queues = Arc::new(AtomicU32::new(0));
        let filtered = !accepted.is_empty() || !forbidden.is_empty();
        let policy = format!("accepted={accepted:?}, forbidden={forbidden:?}");
        let mut options = Worker::options()
            .database(db.database.clone())
            .concurrency(1)
            .poll_interval(Duration::from_millis(20))
            .listen_os_shutdown_signals(false)
            .define_job::<FlaggedJob>()
            .add_extension(executions.clone())
            .local_queue(LocalQueueConfig::builder().size(20).build())
            .on(LocalQueueInit, {
                let counter = local_queues.clone();
                move |_| {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
        for flag in accepted {
            options = options.add_accepted_flag(flag);
        }
        for flag in forbidden {
            options = options.add_forbidden_flag(flag);
        }
        let worker = Arc::new(options.init().await.expect("init worker"));
        let utils = worker.create_utils();
        // The rejected GPU job sits between two eligible jobs in the same
        // named queue, exercising run_once's follow-up claim path too.
        let rows = [
            (None, Some(vec!["windows"])),
            (Some("shared"), Some(vec!["linux"])),
            (Some("shared"), Some(vec!["linux", "gpu"])),
            (
                Some("shared"),
                Some(vec!["linux", "infrastructure_resilient"]),
            ),
            (None, None),
            (None, Some(vec![])),
            (None, Some(vec!["macos"])),
            (None, Some(vec!["LINUX"])),
            (None, Some(vec!["linux", "unrelated"])),
        ];
        for (index, (queue, flags)) in rows.into_iter().enumerate() {
            let value = u32::try_from(index).unwrap();
            utils
                .add_job(
                    FlaggedJob { value },
                    JobSpec {
                        flags: flags.map(|flags| flags.into_iter().map(str::to_owned).collect()),
                        queue_name: queue.map(str::to_owned),
                        priority: Some(i16::try_from(index).unwrap()),
                        ..Default::default()
                    },
                )
                .await
                .expect("seed job");
        }

        if continuous {
            let runner = worker.clone();
            let run = tokio::task::spawn_local(async move { runner.run().await });
            let progress = timeout(Duration::from_secs(5), async {
                loop {
                    if executions.0.lock().unwrap().len() >= expected.len() || run.is_finished() {
                        break;
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            })
            .await;
            let observed = executions.0.lock().unwrap().clone();
            let pending = if progress.is_err() { db.get_jobs().await } else { Vec::new() };
            worker.request_shutdown();
            timeout(Duration::from_secs(5), run)
                .await
                .expect("shutdown worker")
                .expect("worker task")
                .expect("worker run");
            assert!(progress.is_ok(), "eligible jobs must execute ({policy}); expected {expected:?}, observed {observed:?}, pending {pending:?}");
        } else {
            worker.run_once().await.expect("run_once");
        }
        let mut executed = executions.0.lock().unwrap().clone();
        executed.sort_unstable();
        assert_eq!(executed, expected);
        let remaining = db.get_jobs().await;
        assert_eq!(remaining.len(), 9 - expected.len());
        for job in remaining {
            assert_eq!(job.attempts, 0, "rejected job must not consume an attempt");
            assert!(
                job.locked_by.is_none() && job.locked_at.is_none(),
                "rejected job must stay unclaimed"
            );
        }
        for queue in db.get_job_queues().await {
            assert!(
                queue.locked_by.is_none() && queue.locked_at.is_none(),
                "named queues must stay available"
            );
        }
        if continuous && !filtered {
            assert!(
                local_queues.load(Ordering::SeqCst) > 0,
                "default workers must retain LocalQueue"
            );
        } else {
            assert_eq!(
                local_queues.load(Ordering::SeqCst),
                0,
                "filtered workers must use direct claims"
            );
        }
    })
    .await;
}

#[tokio::test]
async fn run_once_routes_any_accepted_label_with_forbidden_veto_and_named_queue_followups() {
    worker_routes_jobs(vec!["linux", "macos"], vec!["gpu"], vec![1, 3, 6, 8], false).await;
    worker_routes_jobs(vec!["linux"], vec![], vec![1, 2, 3, 8], false).await;
    worker_routes_jobs(vec!["linux"], vec!["linux"], vec![], false).await;
    worker_routes_jobs(vec![], vec!["gpu"], vec![0, 1, 3, 4, 5, 6, 7, 8], false).await;
    worker_routes_jobs(vec![], vec![], (0..9).collect(), false).await;
}

#[tokio::test]
async fn continuous_filtered_workers_bypass_local_queue_and_default_workers_keep_it() {
    worker_routes_jobs(vec!["linux", "macos"], vec!["gpu"], vec![1, 3, 6, 8], true).await;
    worker_routes_jobs(vec!["linux"], vec![], vec![1, 2, 3, 8], true).await;
    worker_routes_jobs(vec![], vec!["gpu"], vec![0, 1, 3, 4, 5, 6, 7, 8], true).await;
    worker_routes_jobs(vec![], vec![], (0..9).collect(), true).await;
}

#[tokio::test]
async fn cached_claims_keep_independent_filter_values_shapes_and_time_parameters() {
    with_test_db(|db| async move {
        let special = "label' $4 \\ unicode-é";
        for schema in [Schema::default(), Schema::new("filtered \"tenant")] {
            let utils = WorkerUtils::new(db.database.clone(), schema.clone());
            utils.migrate().await.expect("migrate schema");
            let future = chrono::Utc::now() + chrono::Duration::hours(1);
            let flags = [
                None,
                Some(vec![]),
                Some(vec!["linux"]),
                Some(vec!["windows"]),
                Some(vec!["linux", "gpu"]),
                Some(vec!["linux", "infrastructure_resilient"]),
                Some(vec!["other"]),
                Some(vec![special]),
                Some(vec!["LINUX"]),
            ];
            for (index, flags) in flags.into_iter().enumerate() {
                utils
                    .add_raw_job(
                        "task",
                        json!({"value":index}),
                        JobSpec {
                            flags: flags
                                .map(|flags| flags.into_iter().map(str::to_owned).collect()),
                            priority: Some(i16::try_from(index).unwrap()),
                            run_at: Some(future),
                            ..Default::default()
                        },
                    )
                    .await
                    .expect("seed flag matrix");
            }
            // The SQL API normally turns an empty flag list into NULL. Also
            // exercise an explicitly empty JSON object, with an escaped schema.
            helpers::sql::safe_query(format!(
                "UPDATE {} SET flags = '{{}}'::jsonb WHERE payload->>'value' = '1'",
                schema.private_table("jobs"),
            ))
            .execute(&db.test_pool)
            .await
            .unwrap();
            let tasks = get_tasks_details(&db.database, &schema, vec!["task".into()])
                .await
                .unwrap();
            let policies: Vec<(Vec<&str>, Vec<&str>, Vec<u64>)> = vec![
                (vec![], vec![], (0..9).collect()),
                (vec!["linux"], vec![], vec![2, 4, 5]),
                (vec!["windows"], vec![], vec![3]),
                (vec![special], vec![], vec![7]),
                (vec![], vec!["gpu"], vec![0, 1, 2, 3, 5, 6, 7, 8]),
                (vec!["linux"], vec!["gpu"], vec![2, 5]),
                (vec!["windows"], vec!["gpu"], vec![3]),
                (vec![], vec!["linux"], vec![0, 1, 3, 6, 7, 8]),
                (vec!["linux"], vec!["linux"], vec![]),
                (vec!["LINUX"], vec![], vec![8]),
            ];
            for _ in 0..2 {
                for (accepted, forbidden, expected) in &policies {
                    let accepted: Vec<_> = accepted.iter().map(|s| (*s).to_owned()).collect();
                    let forbidden: Vec<_> = forbidden.iter().map(|s| (*s).to_owned()).collect();
                    for now in [None, Some(future + chrono::Duration::seconds(1))] {
                        for size in [1, 100] {
                            let jobs = batch_get_jobs_with_filter(
                                &db.database,
                                &tasks,
                                &schema,
                                "batch",
                                JobFlagFilter::new(&forbidden, &accepted),
                                size,
                                now,
                            )
                            .await
                            .unwrap();
                            let mut values: Vec<_> = jobs
                                .iter()
                                .map(|j| j.payload()["value"].as_u64().unwrap())
                                .collect();
                            values.sort_unstable();
                            let wanted = if now.is_none() {
                                vec![]
                            } else if size == 1 {
                                expected.iter().take(1).copied().collect()
                            } else {
                                expected.clone()
                            };
                            assert_eq!(values, wanted, "batch policy {accepted:?}/{forbidden:?}");
                            return_jobs(&db.database, &jobs, &schema, "batch")
                                .await
                                .unwrap();
                        }
                        let job = get_job_with_filter(
                            &db.database,
                            &tasks,
                            &schema,
                            "single",
                            JobFlagFilter::new(&forbidden, &accepted),
                            now,
                        )
                        .await
                        .unwrap();
                        let expected = now.and_then(|_| expected.first().copied());
                        assert_eq!(
                            job.as_ref().map(|j| j.payload()["value"].as_u64().unwrap()),
                            expected
                        );
                        if let Some(job) = job {
                            return_jobs(&db.database, &[job], &schema, "single")
                                .await
                                .unwrap();
                        }
                    }
                }
            }
        }
    })
    .await;
}

#[tokio::test]
async fn legacy_job_stream_keeps_its_signature_and_forbidden_filter() {
    with_test_db(|db| async move {
        let worker = db
            .create_worker_options()
            .listen_os_shutdown_signals(false)
            .define_job::<FlaggedJob>()
            .init()
            .await
            .unwrap();
        let utils = worker.create_utils();
        utils
            .add_job(FlaggedJob { value: 1 }, JobSpec::default())
            .await
            .unwrap();
        utils
            .add_job(
                FlaggedJob { value: 2 },
                JobSpec {
                    flags: Some(vec!["blocked".into()]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let jobs: Vec<_> = graphile_worker::streams::job_stream(
            db.database.clone(),
            worker.shutdown_signal().clone(),
            worker.task_details().clone(),
            Schema::default(),
            "legacy".into(),
            vec!["blocked".into()],
            false,
        )
        .collect()
        .await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].payload()["value"], 1);
        let rejected = db
            .get_jobs()
            .await
            .into_iter()
            .find(|j| j.payload["value"] == 2)
            .unwrap();
        assert_eq!(rejected.attempts, 0);
        assert!(rejected.locked_by.is_none());
        return_jobs(&db.database, &jobs, &Schema::default(), "legacy")
            .await
            .unwrap();
        utils
            .add_job(
                FlaggedJob { value: 3 },
                JobSpec {
                    flags: Some(vec!["linux".into()]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let selected: Vec<_> = graphile_worker::streams::job_stream_with_filter(
            db.database.clone(),
            worker.shutdown_signal().clone(),
            worker.task_details().clone(),
            Schema::default(),
            "owned".into(),
            JobFlagFilter::owned(vec!["blocked".into()], vec!["linux".into()]),
            false,
        )
        .collect()
        .await;
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].payload()["value"], 3);
        return_jobs(&db.database, &selected, &Schema::default(), "owned")
            .await
            .unwrap();
    })
    .await;
}
