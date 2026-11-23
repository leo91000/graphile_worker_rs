mod entries;
mod schedule;

pub(crate) use entries::CronEntries;
pub use entries::CronMatcher;

use std::{collections::HashMap, convert::Infallible};

use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::{DbExecutorArg, Schema};
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_shutdown_signal::ShutdownSignal;
use time::{Duration, OffsetDateTime};
use tracing::{debug, error, warn};

use crate::backfill::register_and_backfill_items;
use crate::clock::{Clock, SystemClock};
use crate::sql::ScheduleCronJobError;
use crate::utils::{next_cron_minute, yield_now, ONE_MINUTE};

use self::schedule::emit_tick_and_schedule_jobs;

const MIN_RETRY_DELAY: Duration = Duration::milliseconds(200);
const MAX_RETRY_DELAY: Duration = Duration::seconds(60);

pub async fn cron_main(
    executor: impl DbExecutorArg,
    schema: impl Into<Schema>,
    crontabs: &[Crontab],
    use_local_time: bool,
    shutdown_signal: ShutdownSignal,
    hooks: &HookRegistry,
) -> Result<(), ScheduleCronJobError> {
    CronRunner::new(executor, schema, crontabs, hooks)
        .use_local_time(use_local_time)
        .run(shutdown_signal)
        .await
}

pub struct CronRunner<'a, E, C = SystemClock> {
    executor: E,
    schema: Schema,
    entries: CronEntries<'a>,
    use_local_time: bool,
    hooks: &'a HookRegistry,
    clock: C,
}

impl<'a, E: DbExecutorArg> CronRunner<'a, E, SystemClock> {
    pub fn new(
        executor: E,
        schema: impl Into<Schema>,
        crontabs: &'a [Crontab],
        hooks: &'a HookRegistry,
    ) -> Self {
        Self {
            executor,
            schema: schema.into(),
            entries: CronEntries {
                crontabs,
                matchers: None,
            },
            use_local_time: false,
            hooks,
            clock: SystemClock,
        }
    }
}

impl<'a, E: DbExecutorArg, C: Clock> CronRunner<'a, E, C> {
    pub fn use_local_time(mut self, use_local_time: bool) -> Self {
        self.use_local_time = use_local_time;
        self
    }

    /// Override the timer for selected entries, keyed by their index in the
    /// crontab slice. The same predicate governs ticks, catch-up and backfill.
    pub fn with_matchers(mut self, matchers: &'a HashMap<usize, CronMatcher>) -> Self {
        self.entries.matchers = Some(matchers);
        self
    }

    pub fn with_clock<C2: Clock>(self, clock: C2) -> CronRunner<'a, E, C2> {
        CronRunner {
            executor: self.executor,
            schema: self.schema,
            entries: self.entries,
            use_local_time: self.use_local_time,
            hooks: self.hooks,
            clock,
        }
    }

    pub async fn run(
        mut self,
        shutdown_signal: ShutdownSignal,
    ) -> Result<(), ScheduleCronJobError> {
        let runner = self.run_with_retries();
        futures::pin_mut!(runner);
        futures::future::select(shutdown_signal, runner).await;
        Ok(())
    }

    async fn run_with_retries(&mut self) {
        let mut retry_delay = MIN_RETRY_DELAY;
        loop {
            let error = self.run_until_error(&mut retry_delay).await.unwrap_err();
            error!(
                error = %error,
                retry_delay_ms = retry_delay.whole_milliseconds(),
                "Cron failed; retrying registration and backfill"
            );
            self.clock.sleep_until(self.clock.now() + retry_delay).await;
            retry_delay = (retry_delay + retry_delay / 2i32).min(MAX_RETRY_DELAY);
        }
    }

    async fn run_until_error(
        &mut self,
        retry_delay: &mut Duration,
    ) -> Result<Infallible, ScheduleCronJobError> {
        let start = self.clock.now();
        debug!(start = ?start, "cron:starting");

        register_and_backfill_items(
            &mut self.executor,
            &self.schema,
            self.entries,
            start,
            &self.clock,
            self.use_local_time,
            self.hooks,
        )
        .await?;
        debug!(start = ?start, "cron:started");

        let mut ts = next_cron_minute(&self.clock, start, self.use_local_time);

        loop {
            yield_now().await;
            self.clock.sleep_until(ts).await;

            let ts_delta = self.clock.now() - ts;
            if ts_delta.is_negative() {
                warn!(
                    "Cron fired {}s too early (clock skew?); rescheduling",
                    -ts_delta.whole_seconds()
                );
                continue;
            }
            if ts_delta >= ONE_MINUTE {
                warn!(
                    "Cron fired too late; catching up {}m{}s behind",
                    ts_delta.whole_minutes(),
                    ts_delta.whole_seconds() % 60
                );
            }

            self.emit_tick_and_schedule_jobs(ts).await?;
            *retry_delay = MIN_RETRY_DELAY;
            ts = next_cron_minute(&self.clock, ts, self.use_local_time);
        }
    }

    async fn emit_tick_and_schedule_jobs(
        &mut self,
        ts: OffsetDateTime,
    ) -> Result<(), ScheduleCronJobError> {
        emit_tick_and_schedule_jobs(
            &mut self.executor,
            &self.schema,
            self.entries,
            self.use_local_time,
            self.hooks,
            self.clock.local_time(ts),
        )
        .await
    }
}
