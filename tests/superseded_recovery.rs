use graphile_worker::sql::{
    complete_job::complete_job,
    fail_job::{
        batch::{fail_jobs, FailedJob},
        single::fail_job,
    },
    get_job::get_job,
    return_jobs::{batch::return_jobs, recovery::return_job_for_recovery},
    task_identifiers::get_tasks_details,
};
use graphile_worker::{Job, JobSpec};
use serde_json::json;

mod helpers;

/// Exercises each terminal or attempt-restoring path with the same owned job.
async fn release(db: &helpers::TestDatabase, job: &Job, mode: &str) {
    match mode {
        "sweep" => {
            sqlx::query("select graphile_worker.recover_dead_worker_jobs(array['owner'])")
                .execute(&db.test_pool)
                .await
                .unwrap();
        }
        "return" => return_jobs(
            &db.database,
            std::slice::from_ref(job),
            "graphile_worker",
            "owner",
        )
        .await
        .unwrap(),
        "shutdown" => {
            return_job_for_recovery(&db.database, job, "graphile_worker", "owner", None, None)
                .await
                .unwrap()
        }
        "fail" => fail_job(
            &db.database,
            job,
            "graphile_worker",
            "owner",
            "failed",
            None,
        )
        .await
        .unwrap(),
        "batch_fail" => fail_jobs(
            &db.database,
            &[FailedJob {
                job,
                error: "failed",
            }],
            "graphile_worker",
            "owner",
        )
        .await
        .unwrap(),
        "complete" => complete_job(&db.database, job, "owner", "graphile_worker")
            .await
            .unwrap(),
        _ => panic!("unknown release mode"),
    }
}

/// Prevents obsolete payload revival without losing the owner's queue-unlock path.
#[tokio::test]
async fn replaced_locked_jobs_stay_retired_and_release_their_queues() {
    for mode in [
        "sweep",
        "return",
        "shutdown",
        "fail",
        "batch_fail",
        "complete",
    ] {
        for queue in [None, Some("serial".to_owned())] {
            helpers::with_test_db(move |db| async move {
                db.worker_utils().migrate().await.unwrap();
                let spec = JobSpec { queue_name: queue.clone(), job_key: Some("key".into()), max_attempts: Some(1), ..Default::default() };
                db.add_job("task", json!({"old": true}), spec.clone()).await;
                let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["task".into()]).await.unwrap();
                let old = get_job(&db.database, &tasks, "graphile_worker", "owner", &[], None).await.unwrap().unwrap();
                db.add_job("task", json!({"new": true}), spec).await;
                let rows = db.get_jobs().await;
                let retired = rows.iter().find(|row| row.id == *old.id()).unwrap();
                assert_eq!(retired.locked_by.as_deref(), Some("owner"), "replacement must preserve ownership until release");
                if queue.is_some() {
                    assert!(get_job(&db.database, &tasks, "graphile_worker", "other", &[], None).await.unwrap().is_none());
                }
                release(&db, &old, mode).await;
                let next = get_job(&db.database, &tasks, "graphile_worker", "other", &[], None).await.unwrap().unwrap();
                assert_eq!(next.payload(), &json!({"new": true}), "obsolete payload revived by {mode}");
                complete_job(&db.database, &next, "other", "graphile_worker").await.unwrap();
                assert!(get_job(&db.database, &tasks, "graphile_worker", "other", &[], None).await.unwrap().is_none(), "retired job became available after {mode}");
                assert!(db.get_job_queues().await.iter().all(|q| q.locked_by.is_none()));
                let markers: i64 = sqlx::query_scalar("select count(*) from graphile_worker._private_job_retirements")
                    .fetch_one(&db.test_pool).await.unwrap();
                assert_eq!(markers, i64::from(mode != "complete"), "completion must cascade marker deletion");
                if mode != "complete" {
                    // Explicit administrative rescheduling may revive the old job.
                    sqlx::query("select graphile_worker.reschedule_jobs(array[$1]::bigint[], attempts => 0, run_at => now())")
                        .bind(*old.id()).execute(&db.test_pool).await.unwrap();
                    let revived = get_job(&db.database, &tasks, "graphile_worker", "owner", &[], None).await.unwrap().unwrap();
                    release(&db, &revived, "sweep").await;
                    let retried = get_job(&db.database, &tasks, "graphile_worker", "other", &[], None).await.unwrap().unwrap();
                    assert_eq!(retried.id(), old.id(), "explicitly revived final attempt must recover");
                }
            }).await;
        }
    }
}

/// Distinguishes legitimate final attempts from explicitly retired jobs.
#[tokio::test]
async fn ordinary_final_attempts_are_recovered() {
    for mode in ["sweep", "return", "shutdown"] {
        for queue in [None, Some("serial".to_owned())] {
            helpers::with_test_db(move |db| async move {
                db.worker_utils().migrate().await.unwrap();
                db.add_job(
                    "task",
                    json!({}),
                    JobSpec {
                        queue_name: queue,
                        max_attempts: Some(1),
                        ..Default::default()
                    },
                )
                .await;
                let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["task".into()])
                    .await
                    .unwrap();
                let old = get_job(&db.database, &tasks, "graphile_worker", "owner", &[], None)
                    .await
                    .unwrap()
                    .unwrap();
                release(&db, &old, mode).await;
                let retried = get_job(&db.database, &tasks, "graphile_worker", "other", &[], None)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(retried.id(), old.id());
                assert_eq!(*retried.attempts(), 1);
            })
            .await;
        }
    }
}

/// Keeps administrative retirement intact until an explicit reschedule.
#[tokio::test]
async fn explicitly_removed_or_permanently_failed_jobs_are_not_recovered() {
    for mode in ["sweep", "return", "shutdown"] {
        for removal in ["remove", "permanent"] {
            for queue in [None, Some("serial".to_owned())] {
                helpers::with_test_db(move |db| async move {
                    db.worker_utils().migrate().await.unwrap();
                    db.add_job("task", json!({}), JobSpec { queue_name: queue, job_key: Some("key".into()), ..Default::default() }).await;
                    let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["task".into()]).await.unwrap();
                    let old = get_job(&db.database, &tasks, "graphile_worker", "owner", &[], None).await.unwrap().unwrap();
                    if removal == "remove" {
                        sqlx::query("select graphile_worker.remove_job('key')").execute(&db.test_pool).await.unwrap();
                    } else {
                        // The administration function accepts locks older than four hours.
                        sqlx::query("update graphile_worker._private_jobs set locked_at = now() - interval '5 hours'").execute(&db.test_pool).await.unwrap();
                        sqlx::query("select graphile_worker.permanently_fail_jobs(array[$1]::bigint[], 'operator decision')").bind(*old.id()).execute(&db.test_pool).await.unwrap();
                    }
                    release(&db, &old, mode).await;
                    assert!(get_job(&db.database, &tasks, "graphile_worker", "other", &[], None).await.unwrap().is_none(), "{removal} was undone by {mode}");
                    assert!(db.get_job_queues().await.iter().all(|q| q.locked_by.is_none()));
                    sqlx::query("select graphile_worker.reschedule_jobs(array[$1]::bigint[], attempts => 0, run_at => now())").bind(*old.id()).execute(&db.test_pool).await.unwrap();
                    assert!(get_job(&db.database, &tasks, "graphile_worker", "other", &[], None).await.unwrap().is_some(), "explicit rescheduling remains available");
                }).await;
            }
        }
    }
}

/// Reproduces a stale statement snapshot after waiting for a replacement row lock.
#[tokio::test]
async fn recovery_waits_for_concurrent_retirement_before_restoring_attempts() {
    use std::time::Duration;
    for mode in ["sweep", "return", "shutdown"] {
        helpers::with_test_db(move |db| async move {
            db.worker_utils().migrate().await.unwrap();
            db.add_job("task", json!({"old":true}), JobSpec { job_key: Some("key".into()), queue_name: Some("serial".into()), ..Default::default() }).await;
            let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["task".into()]).await.unwrap();
            let old = get_job(&db.database, &tasks, "graphile_worker", "owner", &[], None).await.unwrap().unwrap();
            let mut tx = db.test_pool.begin().await.unwrap();
            sqlx::query("select graphile_worker.add_job('task', '{\"new\":true}', job_key => 'key', queue_name => 'serial')").execute(&mut *tx).await.unwrap();
            let return_db = db.clone();
            let return_job = old.clone();
            let returning = tokio::spawn(async move { release(&return_db, &return_job, mode).await });
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let blocked: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname = current_database() and pid <> pg_backend_pid() and wait_event_type = 'Lock')").fetch_one(&db.test_pool).await.unwrap();
                    if blocked { break; }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await.expect("return must block behind the replacement row lock");
            tx.commit().await.unwrap();
            tokio::time::timeout(Duration::from_secs(5), returning).await.unwrap().unwrap();
            let rows = db.get_jobs().await;
            let retired = rows.iter().find(|row| row.id == *old.id()).unwrap();
            assert_eq!(retired.attempts, retired.max_attempts, "{mode} must see the committed retirement");
            assert!(retired.locked_by.is_none());
            let next = get_job(&db.database, &tasks, "graphile_worker", "other", &[], None).await.unwrap().unwrap();
            assert_eq!(next.payload(), &json!({"new":true}));
        }).await;
    }
}

/// Requires deliberate revival to see and remove a concurrently committed marker.
#[tokio::test]
async fn explicit_reschedule_clears_concurrently_committed_retirement() {
    use std::time::Duration;
    helpers::with_test_db(|db| async move {
        db.worker_utils().migrate().await.unwrap();
        db.add_job("task", json!({}), JobSpec { max_attempts: Some(1), ..Default::default() }).await;
        let id = db.get_jobs().await[0].id;
        let mut tx = db.test_pool.begin().await.unwrap();
        sqlx::query("select graphile_worker.permanently_fail_jobs(array[$1]::bigint[])")
            .bind(id).execute(&mut *tx).await.unwrap();
        let pool = db.test_pool.clone();
        let rescheduling = tokio::spawn(async move {
            sqlx::query("select graphile_worker.reschedule_jobs(array[$1]::bigint[], attempts => 0)")
                .bind(id).execute(&pool).await.unwrap();
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let blocked: bool = sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname = current_database() and pid <> pg_backend_pid() and wait_event_type = 'Lock')")
                    .fetch_one(&db.test_pool).await.unwrap();
                if blocked { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("reschedule must wait for the retirement row lock");
        tx.commit().await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), rescheduling).await.unwrap().unwrap();
        let markers: i64 = sqlx::query_scalar("select count(*) from graphile_worker._private_job_retirements where id = $1")
            .bind(id).fetch_one(&db.test_pool).await.unwrap();
        assert_eq!(markers, 0, "explicit rescheduling must clear the committed marker");
        let tasks = get_tasks_details(&db.database, "graphile_worker", vec!["task".into()]).await.unwrap();
        let job = get_job(&db.database, &tasks, "graphile_worker", "owner", &[], None).await.unwrap().unwrap();
        release(&db, &job, "return").await;
        assert!(get_job(&db.database, &tasks, "graphile_worker", "other", &[], None).await.unwrap().is_some());
    }).await;
}
