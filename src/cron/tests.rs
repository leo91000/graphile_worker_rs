use chrono::Weekday;
use graphile_worker_crontab_types::{Crontab, CrontabFill, CrontabTimer, CrontabValue, JobKeyMode};
use graphile_worker_ctx::WorkerContext;
use graphile_worker_task_handler::{IntoTaskHandlerResult, TaskHandler};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::Cron;

#[derive(Deserialize, Serialize)]
struct SendDigest {
    message: String,
}

impl TaskHandler for SendDigest {
    const IDENTIFIER: &'static str = "send_digest";

    async fn run(self, _ctx: WorkerContext) -> impl IntoTaskHandlerResult {}
}

#[test]
fn cron_builder_uses_task_identifier_and_options() {
    let crontab = Cron::daily_at::<SendDigest>(8, 30)
        .unwrap()
        .id("daily_digest")
        .fill(CrontabFill::hours(2))
        .max_attempts(3)
        .queue("mail")
        .priority(-1)
        .job_key("daily_digest")
        .job_key_mode(JobKeyMode::PreserveRunAt)
        .payload(SendDigest {
            message: "hello".to_string(),
        })
        .unwrap()
        .build();

    assert_eq!(crontab.task_identifier().as_str(), "send_digest");
    assert_eq!(crontab.identifier(), "daily_digest");
    assert_eq!(crontab.timer().hours(), &vec![CrontabValue::Number(8)]);
    assert_eq!(crontab.timer().minutes(), &vec![CrontabValue::Number(30)]);
    assert_eq!(crontab.options().fill(), &Some(CrontabFill::hours(2)));
    assert_eq!(crontab.options().max(), &Some(3));
    assert_eq!(crontab.options().queue(), &Some("mail".to_string()));
    assert_eq!(crontab.options().priority(), &Some(-1));
    assert_eq!(
        crontab.options().job_key(),
        &Some("daily_digest".to_string())
    );
    assert_eq!(
        crontab.options().job_key_mode(),
        &Some(JobKeyMode::PreserveRunAt)
    );
    assert_eq!(crontab.payload(), &Some(json!({ "message": "hello" })));
}

#[test]
fn cron_builder_converts_into_crontab() {
    let crontab: Crontab = Cron::every_minute::<SendDigest>().into();

    assert_eq!(crontab.task_identifier().as_str(), "send_digest");
    assert_eq!(crontab.timer(), &CrontabTimer::every_minute());
}

#[test]
fn cron_convenience_constructors_build_expected_timers() {
    let every_n = Cron::every_n_minutes::<SendDigest>(15).unwrap().build();
    assert_eq!(every_n.timer(), &CrontabTimer::every_n_minutes(15).unwrap());

    let hourly = Cron::hourly_at::<SendDigest>(10).unwrap().build();
    assert_eq!(hourly.timer(), &CrontabTimer::hourly_at(10).unwrap());

    let daily = Cron::daily_at::<SendDigest>(8, 30).unwrap().build();
    assert_eq!(daily.timer(), &CrontabTimer::daily_at(8, 30).unwrap());

    let weekly = Cron::weekly_on::<SendDigest>(Weekday::Mon, 8, 30)
        .unwrap()
        .build();
    assert_eq!(
        weekly.timer(),
        &CrontabTimer::weekly_on(Weekday::Mon, 8, 30).unwrap()
    );

    let monthly = Cron::monthly_on::<SendDigest>(1, 8, 30).unwrap().build();
    assert_eq!(
        monthly.timer(),
        &CrontabTimer::monthly_on(1, 8, 30).unwrap()
    );

    let yearly = Cron::yearly_on::<SendDigest>(1, 1, 8, 30)
        .unwrap()
        .payload_value(json!({ "message": "manual" }))
        .build();
    assert_eq!(
        yearly.timer(),
        &CrontabTimer::yearly_on(1, 1, 8, 30).unwrap()
    );
    assert_eq!(yearly.payload(), &Some(json!({ "message": "manual" })));
}
