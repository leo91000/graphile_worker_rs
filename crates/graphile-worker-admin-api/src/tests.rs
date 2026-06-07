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
