#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HookResult {
    #[default]
    Continue,
    Skip,
    Fail(String),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_result_default() {
        let result = HookResult::default();
        assert_eq!(result, HookResult::Continue);
    }

    #[test]
    fn test_job_schedule_result_default() {
        let result = JobScheduleResult::default();
        match result {
            JobScheduleResult::Continue(v) => assert_eq!(v, serde_json::Value::Null),
            _ => panic!("Expected Continue variant"),
        }
    }
}
