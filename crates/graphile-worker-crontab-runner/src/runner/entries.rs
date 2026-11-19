use std::{
    collections::HashMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
};

use chrono::NaiveDateTime;
use graphile_worker_crontab_types::Crontab;

use crate::sql::ScheduleCronJobError;

/// A synchronous predicate evaluated on the cron calendar's minute boundaries.
pub type CronMatcher = Arc<dyn Fn(&NaiveDateTime) -> bool + Send + Sync>;

#[derive(Clone, Copy)]
pub(crate) struct CronEntries<'a> {
    pub(crate) crontabs: &'a [Crontab],
    pub(crate) matchers: Option<&'a HashMap<usize, CronMatcher>>,
}

impl CronEntries<'_> {
    pub(crate) fn matches(
        &self,
        index: usize,
        calendar_time: &NaiveDateTime,
    ) -> Result<bool, ScheduleCronJobError> {
        let crontab = &self.crontabs[index];
        match self.matchers.and_then(|matchers| matchers.get(&index)) {
            Some(matcher) => {
                catch_unwind(AssertUnwindSafe(|| matcher(calendar_time))).map_err(|_| {
                    ScheduleCronJobError::MatcherPanicked {
                        identifier: crontab.identifier().to_owned(),
                    }
                })
            }
            None => Ok(crontab.should_run_at(calendar_time)),
        }
    }
}
