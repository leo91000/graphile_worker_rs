use crate::cron::CronBuilder;
use graphile_worker_crontab_parser::{parse_crontab, CrontabParseError};
use graphile_worker_crontab_types::Crontab;
use graphile_worker_task_handler::TaskHandler;

use super::WorkerOptions;

/// Input accepted by [`WorkerOptions::with_cron`].
///
/// Typed cron builders and raw [`Crontab`] values are infallible and return
/// `WorkerOptions` directly. Crontab text is parsed and returns
/// `Result<WorkerOptions, CrontabParseError>`.
pub trait CronInput {
    type Output;

    fn append_to(self, options: WorkerOptions) -> Self::Output;
}

impl CronInput for Crontab {
    type Output = WorkerOptions;

    fn append_to(self, mut options: WorkerOptions) -> Self::Output {
        options.append_crontabs(vec![self]);
        options
    }
}

impl<T: TaskHandler> CronInput for CronBuilder<T> {
    type Output = WorkerOptions;

    fn append_to(self, options: WorkerOptions) -> Self::Output {
        self.build().append_to(options)
    }
}

impl CronInput for &str {
    type Output = Result<WorkerOptions, CrontabParseError>;

    fn append_to(self, mut options: WorkerOptions) -> Self::Output {
        let crontabs = parse_crontab(self)?;
        options.append_crontabs(crontabs);
        Ok(options)
    }
}

impl CronInput for String {
    type Output = Result<WorkerOptions, CrontabParseError>;

    fn append_to(self, options: WorkerOptions) -> Self::Output {
        self.as_str().append_to(options)
    }
}

impl CronInput for &String {
    type Output = Result<WorkerOptions, CrontabParseError>;

    fn append_to(self, options: WorkerOptions) -> Self::Output {
        self.as_str().append_to(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_reference_input_appends_parsed_crontabs() {
        let input = String::from("0 8 * * * send_digest");
        let options = (&input)
            .append_to(WorkerOptions::default())
            .expect("valid crontab");

        let crontabs = options.crontabs.expect("crontabs should be set");

        assert_eq!(crontabs.len(), 1);
        assert_eq!(crontabs[0].task_identifier, "send_digest");
    }
}
