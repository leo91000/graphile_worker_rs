use nom::Parser;

use super::*;
use graphile_worker_crontab_types::{CrontabFill, JobKeyMode};

#[test]
fn test_valid_query() {
    let input = "?fill=4w3d2h1m&priority=-4 foo";
    assert_eq!(
        Ok((
            " foo",
            CrontabOptions {
                fill: Some(CrontabFill {
                    s: 0,
                    m: 1,
                    h: 2,
                    d: 3,
                    w: 4
                }),
                priority: Some(-4),
                ..Default::default()
            }
        )),
        nom_crontab_opts.parse(input)
    );

    let input = "?id=1234dfsd&max=4 bar";
    assert_eq!(
        Ok((
            " bar",
            CrontabOptions {
                id: Some(String::from("1234dfsd")),
                max: Some(4),
                ..Default::default()
            }
        )),
        nom_crontab_opts.parse(input)
    );
}

#[test]
fn test_query_not_preceded_by_question_mark() {
    let input = "fill=4w3d2h1m&priority=-4 foo";

    assert!(nom_crontab_opts.parse(input).is_err());
}

#[test]
fn test_query_with_invalid_fill() {
    let input = "?fill=4w3d2h1m_bruh&priority=-4 foo";
    assert!(nom_crontab_opts.parse(input).is_err());

    let input = "?fill=4w_3d2h1m&priority=-4 foo";
    assert!(nom_crontab_opts.parse(input).is_err());
}

#[test]
fn test_query_with_invalid_query_string() {
    let input = "?max=not-a-number foo";

    assert!(nom_crontab_opts.parse(input).is_err());
}

#[test]
fn parse_job_key() {
    let input = "?job_key=foo";
    assert_eq!(
        Ok((
            "",
            CrontabOptions {
                job_key: Some(String::from("foo")),
                ..Default::default()
            }
        )),
        nom_crontab_opts.parse(input)
    );
}

#[test]
fn parse_job_key_mode_replace() {
    let input = "?job_key_mode=replace";
    assert_eq!(
        Ok((
            "",
            CrontabOptions {
                job_key_mode: Some(JobKeyMode::Replace),
                ..Default::default()
            }
        )),
        nom_crontab_opts.parse(input)
    );
}

#[test]
fn parse_job_key_mode_preserve_run_at() {
    let input = "?job_key_mode=preserve_run_at";
    assert_eq!(
        Ok((
            "",
            CrontabOptions {
                job_key_mode: Some(JobKeyMode::PreserveRunAt),
                ..Default::default()
            }
        )),
        nom_crontab_opts.parse(input)
    );
}
