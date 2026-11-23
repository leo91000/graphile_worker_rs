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
        "2026-01-02T01:04:05.123456Z",
    );
}

#[test]
fn add_job_request_round_trips_signed_years() {
    for timestamp in ["+10000-01-01T00:00:00Z", "-0001-01-01T00:00:00.123456Z"] {
        let request: AddJobRequest = serde_json::from_value(serde_json::json!({
            "identifier": "send_email",
            "run_at": timestamp,
        }))
        .unwrap();

        assert_eq!(serde_json::to_value(request).unwrap()["run_at"], timestamp);
    }
}

#[test]
fn rejects_timestamps_outside_the_utc_range() {
    for timestamp in [
        "+999999-12-31T23:59:59-01:00",
        "-999999-01-01T00:00:00+01:00",
    ] {
        let result = serde_json::from_value::<AddJobRequest>(serde_json::json!({
            "identifier": "send_email",
            "run_at": timestamp,
        }));
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("supported UTC range"));
    }
}

#[test]
fn preserves_legacy_admin_timestamp_inputs() {
    for (timestamp, expected) in [
        ("2026-01-02T03:04:05+0200", "2026-01-02T01:04:05Z"),
        ("2026-01-02T03:04:05 +02:00", "2026-01-02T01:04:05Z"),
        ("2026-01-02T03:04:05+00 30", "2026-01-02T02:34:05Z"),
        ("2026-01-02T03:04:05−00:15", "2026-01-02T03:19:05Z"),
        ("2026-01-02T03:04:05 UTC", "2026-01-02T03:04:05Z"),
        ("2026-1-2T3:4:5Z", "2026-01-02T03:04:05Z"),
        ("2016-12-31T23:59:60.125+0000", "2017-01-01T00:00:00.125Z"),
    ] {
        let request: AddJobRequest = serde_json::from_value(serde_json::json!({
            "identifier": "send_email",
            "run_at": timestamp,
        }))
        .unwrap();
        assert_eq!(serde_json::to_value(request).unwrap()["run_at"], expected);
    }
}

#[test]
fn both_timestamp_parsers_reject_underscore_separator() {
    let timestamp = "2026-01-02_03:04:05Z";
    assert!(crate::parse_rfc3339_utc_timestamp(timestamp).is_err());
    assert!(serde_json::from_value::<AddJobRequest>(serde_json::json!({
        "identifier": "send_email",
        "run_at": timestamp,
    }))
    .is_err());
}

#[test]
fn preserves_timestamp_fractional_precision_and_utc_suffixes() {
    for fraction in ["", ".100", ".100100", ".100100100"] {
        let timestamp = format!("2026-01-02T03:04:05{fraction}Z");
        let parsed = crate::parse_rfc3339_utc_timestamp(&timestamp).unwrap();
        assert_eq!(crate::format_utc_timestamp(parsed).unwrap(), timestamp);
        assert_eq!(
            crate::format_utc_timestamp_with_offset(parsed).unwrap(),
            format!("2026-01-02T03:04:05{fraction}+00:00")
        );
    }
}
