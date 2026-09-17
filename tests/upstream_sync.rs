use graphile_worker::sql::{
    batch_get_jobs::batch_get_jobs, complete_job::complete_job, task_identifiers::get_tasks_details,
};
use graphile_worker::JobSpec;
use helpers::with_test_db;
use serde_json::json;

mod helpers;

#[tokio::test]
async fn batch_fetch_serializes_named_queues_without_dropping_unnamed_jobs() {
    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        let due = chrono::Utc::now() - chrono::Duration::minutes(1);
        for (queue, priority) in [
            (Some("serial"), 2),
            (Some("serial"), 1),
            (Some("other"), 0),
            (None, 0),
            (None, 0),
        ] {
            db.add_job(
                "task",
                json!({}),
                JobSpec {
                    queue_name: queue.map(str::to_owned),
                    priority: Some(priority),
                    run_at: Some(due),
                    ..Default::default()
                },
            )
            .await;
        }
        let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["task".into()])
            .await
            .unwrap();
        let jobs = batch_get_jobs(
            &db.database,
            &tasks,
            "graphile_worker",
            "first",
            &[],
            10,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            jobs.len(),
            4,
            "only one job per named queue, all unnamed jobs"
        );
        assert!(jobs.iter().all(|job| *job.attempts() == 1));
        let serial = jobs.iter().find(|job| *job.priority() == 1).unwrap();
        let waiting = db
            .get_jobs()
            .await
            .into_iter()
            .find(|job| job.priority == 2)
            .unwrap();
        assert_eq!(waiting.attempts, 0);
        assert!(waiting.locked_by.is_none());
        assert!(batch_get_jobs(
            &db.database,
            &tasks,
            "graphile_worker",
            "second",
            &[],
            10,
            None
        )
        .await
        .unwrap()
        .is_empty());
        complete_job(&db.database, serial, "first", "graphile_worker")
            .await
            .unwrap();
        let next = batch_get_jobs(
            &db.database,
            &tasks,
            "graphile_worker",
            "second",
            &[],
            10,
            None,
        )
        .await
        .unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(*next[0].id(), waiting.id);
    })
    .await;
}

#[tokio::test]
async fn repeated_registration_and_insertion_do_not_consume_identities() {
    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        for mode in ["replace", "preserve_run_at", "unsafe_dedupe"] {
            for _ in 0..3 {
                sqlx::query("select graphile_worker.add_job('task', queue_name => 'queue', job_key => 'key', job_key_mode => $1)")
                    .bind(mode).execute(&db.test_pool).await.unwrap();
                get_tasks_details(&db.database, "graphile_worker", vec!["task".into()]).await.unwrap();
            }
        }
        let task_sequence: i64 = sqlx::query_scalar("select pg_sequence_last_value(pg_get_serial_sequence('graphile_worker._private_tasks', 'id')::regclass)").fetch_one(&db.test_pool).await.unwrap();
        let queue_sequence: i64 = sqlx::query_scalar("select pg_sequence_last_value(pg_get_serial_sequence('graphile_worker._private_job_queues', 'id')::regclass)").fetch_one(&db.test_pool).await.unwrap();
        assert_eq!(task_sequence, 1, "existing task must not advance identity");
        assert_eq!(queue_sequence, 1, "existing queue must not advance identity");
        assert_eq!(db.get_jobs().await.len(), 1);
    }).await;
}

#[tokio::test]
async fn cached_fetch_queries_keep_schema_flags_time_and_batch_values_separate() {
    use graphile_worker::sql::{get_job::get_job, return_jobs::batch::return_jobs};
    use graphile_worker::{Schema, WorkerUtils};
    with_test_db(|db| async move {
        let future = chrono::Utc::now() + chrono::Duration::days(365);
        for name in ["graphile_worker", "tenant\"quoted"] {
            let schema = Schema::new(name);
            let utils = WorkerUtils::new(db.database.clone(), schema.clone());
            utils.migrate().await.unwrap();
            for flags in [None, Some(vec!["blocked".to_owned()])] {
                utils
                    .add_raw_job(
                        "task",
                        json!({}),
                        JobSpec {
                            flags,
                            run_at: Some(future),
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap();
            }
            let tasks = get_tasks_details(&db.database, &schema, vec!["task".into()])
                .await
                .unwrap();
            for _ in 0..2 {
                for flags in [vec![], vec!["blocked".to_owned()]] {
                    for now in [None, Some(future + chrono::Duration::seconds(1))] {
                        for size in [1, 10] {
                            let jobs = batch_get_jobs(
                                &db.database,
                                &tasks,
                                &schema,
                                "owner",
                                &flags,
                                size,
                                now,
                            )
                            .await
                            .unwrap();
                            let expected = if now.is_none() {
                                0
                            } else if size == 1 || !flags.is_empty() {
                                1
                            } else {
                                2
                            };
                            assert_eq!(jobs.len(), expected);
                            return_jobs(&db.database, &jobs, &schema, "owner")
                                .await
                                .unwrap();
                        }
                        let job = get_job(&db.database, &tasks, &schema, "single", &flags, now)
                            .await
                            .unwrap();
                        assert_eq!(job.is_some(), now.is_some());
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
async fn batched_failure_accepts_job_ids_above_int32() {
    use graphile_worker::sql::fail_job::batch::{fail_jobs, FailedJob};
    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        sqlx::query("select setval(pg_get_serial_sequence('graphile_worker._private_jobs', 'id'), 2147483648, false)").execute(&db.test_pool).await.unwrap();
        for queue in [None, Some("serial".to_owned())] {
            db.add_job("task", json!({}), JobSpec { queue_name: queue, ..Default::default() }).await;
        }
        let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["task".into()]).await.unwrap();
        let jobs = batch_get_jobs(&db.database, &tasks, "graphile_worker", "owner", &[], 10, None).await.unwrap();
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|job| *job.id() > i32::MAX as i64));
        for job in &jobs {
            fail_jobs(&db.database, &[FailedJob { job, error: "retry" }], "graphile_worker", "owner").await.unwrap();
        }
        let failed = db.get_jobs().await;
        assert!(failed.iter().all(|job| job.locked_by.is_none() && job.last_error.as_deref() == Some("retry") && job.attempts == 1));
        assert!(db.get_job_queues().await.iter().all(|queue| queue.locked_by.is_none()));
    }).await;
}

#[tokio::test]
async fn concurrent_batch_fetches_share_only_unnamed_work() {
    with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        for _ in 0..10 {
            for queue in [None, Some("serial".to_owned())] {
                db.add_job(
                    "task",
                    json!({}),
                    JobSpec {
                        queue_name: queue,
                        ..Default::default()
                    },
                )
                .await;
            }
        }
        let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["task".into()])
            .await
            .unwrap();
        let (first, second) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(
                batch_get_jobs(
                    &db.database,
                    &tasks,
                    "graphile_worker",
                    "one",
                    &[],
                    100,
                    None
                ),
                batch_get_jobs(
                    &db.database,
                    &tasks,
                    "graphile_worker",
                    "two",
                    &[],
                    100,
                    None
                )
            )
        })
        .await
        .expect("concurrent fetches must not deadlock");
        let claimed: Vec<_> = first.unwrap().into_iter().chain(second.unwrap()).collect();
        assert_eq!(claimed.len(), 11);
        assert_eq!(
            claimed
                .iter()
                .filter(|job| job.job_queue_id().is_some())
                .count(),
            1
        );
        let ids: std::collections::HashSet<_> = claimed.iter().map(|job| job.id()).collect();
        assert_eq!(ids.len(), claimed.len());
        assert_eq!(
            db.get_jobs()
                .await
                .iter()
                .filter(|job| job.attempts == 0)
                .count(),
            9
        );
    })
    .await;
}
