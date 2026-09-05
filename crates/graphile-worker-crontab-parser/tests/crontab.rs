use graphile_worker_crontab_parser::parse_crontab;
use serde_json::json;

#[test]
fn nested_identifiers_preserve_options_payload_and_following_entries() {
    let entries = parse_crontab(
        "* * * * * emails/daily ?id=digest&fill=1h&job_key=daily {urgent:true}\n* * * * * _maintenance/cleanup",
    )
    .unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].task_identifier(), "emails/daily");
    assert_eq!(entries[0].identifier(), "digest");
    assert_eq!(
        entries[0].options().fill().as_ref().unwrap().to_secs(),
        3600
    );
    assert_eq!(entries[0].options().job_key().as_deref(), Some("daily"));
    assert_eq!(entries[0].payload(), &Some(json!({"urgent": true})));
    assert_eq!(entries[1].task_identifier(), "_maintenance/cleanup");
}

#[test]
fn accepts_blank_lines_comments_crlf_and_trailing_spaces() {
    let entries = parse_crontab(
        " \t\r\n  # schedules\r\n\r\n  * * * * * first  \t\r\n \t\r\n# middle\r\n* * * * * second\r\n  # end",
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].task_identifier(), "first");
    assert_eq!(entries[1].task_identifier(), "second");
    assert!(parse_crontab("").unwrap().is_empty());
    assert!(parse_crontab(" \n# comment\n\t").unwrap().is_empty());
}

#[test]
fn invalid_entries_reject_the_whole_crontab_with_a_location() {
    for invalid in [
        "* * * * * task.invalid",
        "* * * * * task trailing",
        "* * * * * task ?max=invalid",
        "* * * * * task ?fill=1h_invalid",
        "* * * * * task {invalid",
        "60 * * * * task",
        "not a schedule",
    ] {
        let input = format!("# start\n* * * * * valid\n{invalid}\n* * * * * later");
        let error = parse_crontab(&input).expect_err(invalid);
        assert!(error.msg.contains("line 3, column "), "{error}");
        assert!(parse_crontab(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn reports_column_of_unexpected_suffix() {
    let error = parse_crontab("* * * * * task!").unwrap_err();
    assert!(error.msg.contains("line 1, column 15:"), "{error}");
}
