use super::*;

#[tokio::test]
async fn add_job_rejects_job_key_mode_without_key() {
    let pool = lazy_pool();
    let database: graphile_worker::Database = pool.clone().into();
    let state = Arc::new(AppState {
        pool,
        utils: WorkerUtils::new(database, "graphile_worker".to_string()),
        schema: Schema::new("graphile_worker"),
        schema_name: "graphile_worker".to_string(),
        auth: AdminAuthConfig::None,
        csrf_token: "csrf".to_string(),
        read_only: false,
    });

    let error = add_job(
        State(state),
        Json(AddJobRequest {
            identifier: "send_email".to_string(),
            payload: serde_json::json!({}),
            queue: None,
            run_at: None,
            max_attempts: None,
            key: None,
            job_key_mode: Some(JobKeyModeRequest::Replace),
            priority: None,
            flags: None,
        }),
    )
    .await
    .expect_err("request should be rejected before reaching the database");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("key"));
}

#[test]
fn job_filters_ignore_whitespace_only_search() {
    let args = ListJobsParams {
        state: JobState::All,
        identifier: None,
        queue: None,
        search: Some("   ".to_string()),
        limit: default_limit(),
        offset: 0,
    };
    let mut query = QueryBuilder::<Postgres>::new("where true");

    apply_job_filters(&mut query, &args);

    assert_eq!(query.sql(), "where true");
}
