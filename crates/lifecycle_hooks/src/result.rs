use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HookResult {
    #[default]
    Continue,
    Skip,
    Fail(String),
    Retry {
        delay: Duration,
    },
}

#[derive(Debug, Clone)]
pub enum JobScheduleResult {
    Continue(serde_json::Value),
    Skip,
    Fail(String),
}

impl Default for JobScheduleResult {
    fn default() -> Self {
        JobScheduleResult::Continue(serde_json::Value::Null)
    }
}
