use crate::jobs::{AddJobRequest, JobActionResponse};

#[test]
fn job_action_response_defaults_missing_jobs() {
    let response: JobActionResponse =
        serde_json::from_str(r#"{"message":"Completed 1 job(s)"}"#).unwrap();

    assert_eq!(response.message, "Completed 1 job(s)");
    assert!(response.jobs.is_empty());
}

#[test]
fn add_job_request_defaults_payload_to_empty_object() {
    let request: AddJobRequest = serde_json::from_str(r#"{"identifier":"send_email"}"#)
        .expect("minimal add job request should deserialize");

    assert_eq!(request.identifier, "send_email");
    assert_eq!(request.payload, serde_json::json!({}));
}

#[test]
fn add_job_request_accepts_existing_rfc3339_timestamps() {
    let request: AddJobRequest = serde_json::from_str(
        r#"{"identifier":"send_email","run_at":"2026-01-02T03:04:05.123456+02:00"}"#,
    )
    .unwrap();

    let timestamp = request.run_at.unwrap();
    assert_eq!(
        timestamp
            .to_offset(time::UtcOffset::UTC)
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap(),
        "2026-01-02T01:04:05.123456Z",
    );
    assert_eq!(
        serde_json::to_value(request).unwrap()["run_at"],
        "2026-01-02T03:04:05.123456+02:00",
    );
}
