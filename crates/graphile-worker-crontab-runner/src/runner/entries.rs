use std::{
    collections::HashMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
};

use graphile_worker_crontab_types::Crontab;
use time::PrimitiveDateTime;

use tracing::warn;

/// A synchronous predicate evaluated on the cron calendar's minute boundaries.
pub type CronMatcher = Arc<dyn Fn(&PrimitiveDateTime) -> bool + Send + Sync>;

#[derive(Clone, Copy)]
pub(crate) struct CronEntries<'a> {
    pub(crate) crontabs: &'a [Crontab],
    pub(crate) matchers: Option<&'a HashMap<usize, CronMatcher>>,
}

impl CronEntries<'_> {
    pub(crate) fn matches(&self, index: usize, calendar_time: &PrimitiveDateTime) -> bool {
        let crontab = &self.crontabs[index];
        let Some(matcher) = self.matchers.and_then(|matchers| matchers.get(&index)) else {
            return crontab.should_run_at(calendar_time);
        };
        match catch_unwind(AssertUnwindSafe(|| matcher(calendar_time))) {
            Ok(matches) => matches,
            Err(_) => {
                warn!(identifier = crontab.identifier(), %calendar_time, "Cron matcher panicked");
                false
            }
        }
    }
}
