use std::time::Duration;

use derive_builder::Builder;

#[derive(Debug, Clone, Builder)]
#[builder(build_fn(private, name = "build_internal"), pattern = "owned")]
pub struct RefetchDelayConfig {
    #[builder(default = "Duration::from_millis(100)")]
    pub duration: Duration,
    #[builder(default = "0")]
    pub threshold: usize,
    #[builder(default, setter(strip_option))]
    pub max_abort_threshold: Option<usize>,
}

impl Default for RefetchDelayConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl RefetchDelayConfig {
    pub fn builder() -> RefetchDelayConfigBuilder {
        RefetchDelayConfigBuilder::default()
    }

    pub fn with_duration(self, duration: Duration) -> Self {
        Self { duration, ..self }
    }

    pub fn with_threshold(self, threshold: usize) -> Self {
        Self { threshold, ..self }
    }

    pub fn with_max_abort_threshold(self, max_abort_threshold: usize) -> Self {
        Self {
            max_abort_threshold: Some(max_abort_threshold),
            ..self
        }
    }
}

impl RefetchDelayConfigBuilder {
    pub fn build(self) -> RefetchDelayConfig {
        self.build_internal()
            .expect("All fields have defaults, build should never fail")
    }
}
