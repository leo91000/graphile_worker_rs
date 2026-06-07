mod schedule;

use std::cmp::Ordering;

use chrono::prelude::*;
use futures::FutureExt;
use graphile_worker_crontab_types::Crontab;
use graphile_worker_database::{DbExecutorArg, Schema};
use graphile_worker_lifecycle_hooks::HookRegistry;
use graphile_worker_shutdown_signal::ShutdownSignal;
use tracing::{debug, warn};

use crate::backfill::register_and_backfill_items;
use crate::clock::{Clock, SystemClock};
use crate::sql::ScheduleCronJobError;
use crate::utils::{round_date_minute, ONE_MINUTE};

use self::schedule::emit_tick_and_schedule_jobs;

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
    crontabs: &'a [Crontab],
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
            crontabs,
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

    pub fn with_clock<C2: Clock>(self, clock: C2) -> CronRunner<'a, E, C2> {
        CronRunner {
            executor: self.executor,
            schema: self.schema,
            crontabs: self.crontabs,
            use_local_time: self.use_local_time,
            hooks: self.hooks,
            clock,
        }
    }

    pub async fn run(
        mut self,
        mut shutdown_signal: ShutdownSignal,
    ) -> Result<(), ScheduleCronJobError> {
        let start = self.clock.now();
        debug!(start = ?start, "cron:starting");

        register_and_backfill_items(
            &mut self.executor,
            &self.schema,
            self.crontabs,
            &start,
            self.use_local_time,
        )
        .await?;
        debug!(start = ?start, "cron:started");

        let mut ts = round_date_minute(start, true);

        loop {
            let should_shutdown = {
                let sleep = self.clock.sleep_until(ts).fuse();
                let shutdown = (&mut shutdown_signal).fuse();
                futures::pin_mut!(sleep, shutdown);

                futures::select_biased! {
                    _ = shutdown => true,
                    _ = sleep => false,
                }
            };

            if should_shutdown {
                break Ok(());
            }

            let current_ts = round_date_minute(self.clock.now(), false);
            let ts_delta = current_ts - ts;

            match ts_delta.num_minutes().cmp(&0) {
                Ordering::Less => {
                    warn!(
                        "Cron fired {}s too early (clock skew?); rescheduling",
                        -ts_delta.num_seconds()
                    );
                    continue;
                }
                Ordering::Greater => {
                    warn!(
                        "Cron fired too late; catching up {}m{}s behind",
                        ts_delta.num_minutes(),
                        ts_delta.num_seconds() % 60
                    );
                }
                _ => {}
            }

            self.emit_tick_and_schedule_jobs(ts).await?;
            ts += *ONE_MINUTE;
        }
    }

    async fn emit_tick_and_schedule_jobs(
        &mut self,
        ts: DateTime<Local>,
    ) -> Result<(), ScheduleCronJobError> {
        emit_tick_and_schedule_jobs(
            &mut self.executor,
            &self.schema,
            self.crontabs,
            self.use_local_time,
            self.hooks,
            ts,
        )
        .await
    }
}
