use super::*;

#[test]
fn worker_options_with_cron_accepts_crontab_text() {
    let _ = WorkerOptions::default()
        .with_cron("* * * * * builder_job")
        .expect("Failed to parse crontab string literal");

    let _ = WorkerOptions::default()
        .with_cron(String::from("* * * * * builder_job"))
        .expect("Failed to parse owned crontab string");
}

#[test]
#[allow(deprecated)]
fn worker_options_deprecated_with_crontab_still_parses_text() {
    let _ = WorkerOptions::default()
        .with_crontab("* * * * * builder_job")
        .expect("Failed to parse deprecated crontab string");
}
