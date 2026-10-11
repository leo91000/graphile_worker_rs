use chrono::{Duration, Utc};
use futures::{FutureExt, StreamExt};
use graphile_worker::sql::{
    batch_get_jobs::batch_get_jobs_with_filter, get_job::get_job_with_filter,
    return_jobs::batch::return_jobs, task_identifiers::get_tasks_details,
};
use graphile_worker::{JobFlagFilter, JobSpec, Schema, WorkerUtils};
use serde_json::json;

mod helpers;
use helpers::with_test_db;

/// Alternates every filter shape, clock mode, and allowlist value against the
/// same jobs, guarding SQL cache isolation and parameter positions in both claims.
#[tokio::test]
async fn allowlist_claims_match_keys_and_keep_rejected_jobs_untouched() {
    with_test_db(|db| async move {
        let special = "quote' $5 \\ é";
        for schema in [Schema::default(), Schema::new("allowlist \"tenant")] {
            let utils = WorkerUtils::new(db.database.clone(), schema.clone());
            utils.migrate().await.unwrap();
            let future = Utc::now() + Duration::hours(1);
            let shapes = [
                None,
                Some(json!({})),
                Some(json!({"linux":true})),
                Some(json!({"linux":true,"gpu":true})),
                Some(json!({"linux":true,"infrastructure_resilient":true})),
                Some(json!({"windows":true})),
                Some(json!({"LINUX":true})),
                Some(json!({(special):true})),
                Some(json!({"linux":false})),
                Some(json!(["linux"])),
                Some(json!("linux")),
                Some(json!(123)),
                Some(json!(null)),
                Some(json!({"":true})),
                Some(json!({"linux":{"nested":true}})),
            ];
            for (index, flags) in shapes.iter().enumerate() {
                let job = utils.add_raw_job("task", json!({"value":index}), JobSpec {
                    run_at: Some(future), priority: Some(index as i16),
                    queue_name: Some(format!("queue-{index}")), ..Default::default()
                }).await.unwrap();
                helpers::sql::safe_query(format!("UPDATE {} SET flags = $1::jsonb WHERE id = $2", schema.private_table("jobs")))
                    .bind(flags.clone()).bind(*job.id()).execute(&db.test_pool).await.unwrap();
            }
            let tasks = get_tasks_details(&db.database, &schema, vec!["task".into()]).await.unwrap();
            let allowlists = [None, Some(vec![]), Some(vec!["linux"]), Some(vec!["linux","gpu","infrastructure_resilient"]), Some(vec![special]), Some(vec!["linux","linux"]), Some(vec![""])];
            for _ in 0..2 {
                for allowlist in &allowlists {
                    for accepted in [vec![], vec!["linux"]] {
                        for forbidden in [vec![], vec!["gpu"]] {
                            let allowed = allowlist.as_ref().map(|flags| flags.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
                            let accepted = accepted.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
                            let forbidden = forbidden.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
                            let mut filter = JobFlagFilter::new(&forbidden, &accepted);
                            if let Some(allowed) = &allowed { filter = filter.with_allowlist(allowed.as_slice()); }
                            assert_eq!(filter.allowlist(), allowed.as_deref());
                            let keys = |flags: &Option<serde_json::Value>, flag: &str| match flags {
                                Some(serde_json::Value::Object(object)) => object.contains_key(flag),
                                Some(serde_json::Value::Array(array)) => array.iter().any(|value| value.as_str() == Some(flag)),
                                Some(serde_json::Value::String(value)) => value == flag,
                                _ => false,
                            };
                            let expected = shapes.iter().enumerate().filter_map(|(index, flags)| {
                                if forbidden.iter().any(|flag| keys(flags, flag)) { return None; }
                                if !accepted.is_empty() && !accepted.iter().any(|flag| keys(flags, flag)) { return None; }
                                if let Some(allowed) = &allowed {
                                    match flags {
                                        None => {},
                                        Some(serde_json::Value::Object(object)) if object.keys().all(|key| allowed.contains(key)) => {},
                                        _ => return None,
                                    }
                                }
                                Some(index as u64)
                            }).collect::<Vec<_>>();
                            for now in [None, Some(future + Duration::seconds(1))] {
                                for size in [1, 100] {
                                    let jobs = batch_get_jobs_with_filter(&db.database, &tasks, &schema, "batch", filter.as_borrowed(), size, now).await.unwrap();
                                    let mut values = jobs.iter().map(|job| job.payload()["value"].as_u64().unwrap()).collect::<Vec<_>>();
                                    values.sort_unstable();
                                    let wanted = if now.is_none() { vec![] } else { expected.iter().take(size as usize).copied().collect() };
                                    assert_eq!(values, wanted, "allow={allowed:?}, accepted={accepted:?}, forbidden={forbidden:?}");
                                    return_jobs(&db.database, &jobs, &schema, "batch").await.unwrap();
                                }
                                let job = get_job_with_filter(&db.database, &tasks, &schema, "single", filter.as_borrowed(), now).await.unwrap();
                                assert_eq!(job.as_ref().map(|job| job.payload()["value"].as_u64().unwrap()), now.and_then(|_| expected.first().copied()));
                                if let Some(job) = job { return_jobs(&db.database, &[job], &schema, "single").await.unwrap(); }
                            }
                        }
                    }
                }
            }
            // Claim/release tests restore all jobs. Check directly to also verify
            // each named queue's persisted ownership is returned.
            let locked: i64 = helpers::sql::safe_query_scalar(format!("SELECT count(*) FROM {} WHERE attempts <> 0 OR locked_by IS NOT NULL OR locked_at IS NOT NULL", schema.private_table("jobs"))).fetch_one(&db.test_pool).await.unwrap();
            assert_eq!(locked, 0);
            let locked_queues: i64 = helpers::sql::safe_query_scalar(format!("SELECT count(*) FROM {} WHERE locked_by IS NOT NULL OR locked_at IS NOT NULL", schema.private_table("job_queues"))).fetch_one(&db.test_pool).await.unwrap();
            assert_eq!(locked_queues, 0);
        }
    }).await;
}

#[tokio::test]
/// A pending replacement becomes eligible while a second rejected job detects
/// loss of the owned allowlist during asynchronous stream polling.
async fn owned_allowlist_is_preserved_by_stream_and_pending_job_replacement() {
    with_test_db(|db| async move {
        let schema = Schema::default();
        let utils = WorkerUtils::new(db.database.clone(), schema.clone());
        utils.migrate().await.unwrap();
        let spec = |flags: Vec<&str>| JobSpec {
            flags: Some(flags.into_iter().map(str::to_owned).collect()),
            job_key: Some("build".into()),
            ..Default::default()
        };
        let original = utils
            .add_raw_job("build", json!({}), spec(vec!["linux", "kvm"]))
            .await
            .unwrap();
        let tasks = get_tasks_details(&db.database, &schema, vec!["build".into()])
            .await
            .unwrap();
        let filter = JobFlagFilter::owned(vec![], vec![]).with_allowlist(vec!["linux".into()]);
        assert!(get_job_with_filter(
            &db.database,
            &tasks,
            &schema,
            "worker",
            filter.as_borrowed(),
            None
        )
        .await
        .unwrap()
        .is_none());
        let replaced = utils
            .add_raw_job("build", json!({}), spec(vec!["linux"]))
            .await
            .unwrap();
        assert_eq!(original.id(), replaced.id());
        let rejected = utils
            .add_raw_job(
                "build",
                json!({}),
                JobSpec {
                    flags: Some(vec!["linux".into(), "gpu".into()]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let stream = graphile_worker::streams::job_stream_with_filter(
            db.database.clone(),
            futures::future::pending::<()>().boxed().shared(),
            tasks.into(),
            schema,
            "worker".into(),
            filter,
            false,
        );
        futures::pin_mut!(stream);
        let job = stream.next().await.expect("replacement is eligible");
        assert_eq!(job.id(), original.id());
        assert!(stream.next().await.is_none());
        let pending = db
            .get_jobs()
            .await
            .into_iter()
            .find(|job| job.id == *rejected.id())
            .unwrap();
        assert_eq!(pending.attempts, 0);
        assert!(pending.locked_by.is_none() && pending.locked_at.is_none());
    })
    .await;
}
