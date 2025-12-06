use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HookResult {
    #[default]
    Continue,
    Skip,
    Fail(String),
    Retry { delay: Duration },
}
