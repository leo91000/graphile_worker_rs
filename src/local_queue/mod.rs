use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures::FutureExt;
use graphile_worker_database::Database;
use graphile_worker_job::Job;
pub use graphile_worker_lifecycle_hooks::LocalQueueMode;
use graphile_worker_lifecycle_hooks::{
    HookRegistry, LocalQueueGetJobsCompleteContext, LocalQueueInitContext,
    LocalQueueRefetchDelayAbortContext, LocalQueueRefetchDelayExpiredContext,
    LocalQueueRefetchDelayStartContext, LocalQueueReturnJobsContext, LocalQueueSetModeContext,
};
use graphile_worker_runtime as runtime;
use graphile_worker_shutdown_signal::ShutdownSignal;
use rand::RngExt;
use thiserror::Error;

use crate::background_tasks::TaskSlot;
use crate::streams::JobSignalSender;
use config::{calculate_retry_delay, RETURN_JOBS_RETRY_OPTIONS};
use tracing::{debug, error, trace, warn};

use crate::sql::batch_get_jobs::batch_get_jobs;
use crate::sql::get_job::get_job;
use crate::sql::return_jobs::return_jobs;
use crate::sql::task_identifiers::SharedTaskDetails;

mod config;
pub use config::{
    LocalQueueConfig, LocalQueueConfigBuilder, RefetchDelayConfig, RefetchDelayConfigBuilder,
};

#[derive(Debug, Error)]
pub enum LocalQueueError {
    #[error("Failed to return jobs to database: {0}")]
    ReturnJobsError(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::errors::GraphileWorkerError),
}

struct RefetchDelayState {
    active: AtomicBool,
    fetch_on_complete: AtomicBool,
    counter: AtomicUsize,
    abort_threshold: runtime::RwLock<usize>,
    abort_notify: runtime::Notify,
}

impl Default for RefetchDelayState {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(false),
            fetch_on_complete: AtomicBool::new(false),
            counter: AtomicUsize::new(0),
            abort_threshold: runtime::RwLock::new(usize::MAX),
            abort_notify: runtime::Notify::new(),
        }
    }
}

struct LocalQueueState {
    mode: runtime::RwLock<LocalQueueMode>,
    job_queue: runtime::Mutex<VecDeque<Job>>,
    job_signal_sender: JobSignalSender,
    fetch_in_progress: AtomicBool,
    fetch_again: AtomicBool,
    refetch_delay: RefetchDelayState,
    state_notify: runtime::Notify,
    run_task: TaskSlot,
    shutdown_task: TaskSlot,
    refetch_delay_task: TaskSlot,
    ttl_timer_task: TaskSlot,
    run_complete_notify: runtime::Notify,
    config: LocalQueueConfig,
    database: Database,
    escaped_schema: String,
    worker_id: String,
    task_details: SharedTaskDetails,
    poll_interval: Duration,
    continuous: bool,
    hooks: Arc<HookRegistry>,
    use_local_time: bool,
}

impl LocalQueueState {
    fn new(params: LocalQueueParams) -> Self {
        Self {
            mode: runtime::RwLock::new(LocalQueueMode::Starting),
            job_queue: runtime::Mutex::new(VecDeque::new()),
            job_signal_sender: params.job_signal_sender,
            fetch_in_progress: AtomicBool::new(false),
            fetch_again: AtomicBool::new(false),
            refetch_delay: RefetchDelayState::default(),
            state_notify: runtime::Notify::new(),
            run_task: TaskSlot::empty("local_queue_run"),
            shutdown_task: TaskSlot::empty("local_queue_shutdown"),
            refetch_delay_task: TaskSlot::empty("local_queue_refetch_delay"),
            ttl_timer_task: TaskSlot::empty("local_queue_ttl"),
            run_complete_notify: runtime::Notify::new(),
            config: params.config,
            database: params.database,
            escaped_schema: params.escaped_schema,
            worker_id: params.worker_id,
            task_details: params.task_details,
            poll_interval: params.poll_interval,
            continuous: params.continuous,
            hooks: params.hooks,
            use_local_time: params.use_local_time,
        }
    }
}

#[derive(Clone)]
pub struct LocalQueue(Arc<LocalQueueState>);

impl From<LocalQueueState> for LocalQueue {
    fn from(state: LocalQueueState) -> Self {
        Self(Arc::new(state))
    }
}

impl From<LocalQueueParams> for LocalQueue {
    fn from(params: LocalQueueParams) -> Self {
        LocalQueueState::new(params).into()
    }
}

pub struct LocalQueueParams {
    pub config: LocalQueueConfig,
    pub database: Database,
    pub escaped_schema: String,
    pub worker_id: String,
    pub task_details: SharedTaskDetails,
    pub poll_interval: Duration,
    pub continuous: bool,
    pub shutdown_signal: Option<ShutdownSignal>,
    pub hooks: Arc<HookRegistry>,
    pub job_signal_sender: JobSignalSender,
    pub use_local_time: bool,
}

impl LocalQueue {
    // Helper methods for atomic operations

    fn try_start_fetch(&self) -> bool {
        self.0
            .fetch_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    fn end_fetch(&self) {
        self.0.fetch_in_progress.store(false, Ordering::SeqCst);
    }

    fn is_fetch_in_progress(&self) -> bool {
        self.0.fetch_in_progress.load(Ordering::SeqCst)
    }

    fn set_fetch_again(&self, value: bool) {
        self.0.fetch_again.store(value, Ordering::SeqCst);
    }

    fn take_fetch_again(&self) -> bool {
        self.0.fetch_again.swap(false, Ordering::SeqCst)
    }

    fn is_refetch_delay_active(&self) -> bool {
        self.0.refetch_delay.active.load(Ordering::SeqCst)
    }

    fn set_refetch_delay_active(&self, value: bool) {
        self.0.refetch_delay.active.store(value, Ordering::SeqCst);
    }

    fn reset_refetch_delay_counter(&self) {
        self.0.refetch_delay.counter.store(0, Ordering::SeqCst);
    }

    fn increment_refetch_delay_counter(&self, count: usize) {
        self.0
            .refetch_delay
            .counter
            .fetch_add(count, Ordering::SeqCst);
    }

    fn get_refetch_delay_counter(&self) -> usize {
        self.0.refetch_delay.counter.load(Ordering::SeqCst)
    }

    fn set_refetch_delay_fetch_on_complete(&self, value: bool) {
        self.0
            .refetch_delay
            .fetch_on_complete
            .store(value, Ordering::SeqCst);
    }

    fn take_refetch_delay_fetch_on_complete(&self) -> bool {
        self.0
            .refetch_delay
            .fetch_on_complete
            .swap(false, Ordering::SeqCst)
    }

    pub fn new(params: LocalQueueParams) -> Self {
        if let Some(ref refetch_delay) = params.config.refetch_delay {
            if refetch_delay.duration > params.poll_interval {
                panic!(
                    "refetch_delay.duration ({:?}) must not be larger than poll_interval ({:?})",
                    refetch_delay.duration, params.poll_interval
                );
            }
        }

        if params.config.size == 0 {
            panic!("local_queue.size must be greater than 0");
        }

        if params.config.queue_count == 0 {
            panic!("local_queue.queue_count must be greater than 0");
        }

        if params.config.size > i32::MAX as usize {
            panic!(
                "local_queue.size ({}) must not exceed i32::MAX ({})",
                params.config.size,
                i32::MAX
            );
        }

        let shutdown_signal = params.shutdown_signal.clone();
        let queue: LocalQueue = params.into();

        let queue_clone = queue.clone();
        let run_task = runtime::spawn(async move {
            queue_clone.run().await;
        });
        queue.0.run_task.replace_abort(run_task);

        if let Some(signal) = shutdown_signal {
            let queue_for_shutdown = queue.clone();
            let shutdown_task = runtime::spawn(async move {
                signal.await;
                if let Err(e) = queue_for_shutdown.release().await {
                    warn!(error = %e, "Error releasing LocalQueue on shutdown");
                }
            });
            queue.0.shutdown_task.replace_abort(shutdown_task);
        }

        queue
    }

    async fn run(&self) {
        self.0
            .hooks
            .emit(LocalQueueInitContext {
                worker_id: self.0.worker_id.clone(),
            })
            .await;

        self.set_mode(LocalQueueMode::Polling).await;
        self.schedule_fetch().await;

        self.0.run_complete_notify.notify_one();
    }

    async fn schedule_fetch(&self) {
        loop {
            let mode = *self.0.mode.read().await;

            if mode == LocalQueueMode::Released {
                break;
            }

            let can_fetch = mode == LocalQueueMode::Polling
                && !self.is_fetch_in_progress()
                && !self.is_refetch_delay_active();

            if !can_fetch {
                self.0.state_notify.notified().await;
                continue;
            }

            self.fetch().await;

            let mode = *self.0.mode.read().await;
            if mode == LocalQueueMode::Polling && self.0.continuous {
                let should_fetch_again = self.take_fetch_again();
                let refetch_delay_wants_fetch = self.take_refetch_delay_fetch_on_complete();

                if !should_fetch_again && !refetch_delay_wants_fetch {
                    let sleep = runtime::sleep(self.0.poll_interval).fuse();
                    let notified = self.0.state_notify.notified().fuse();
                    futures::pin_mut!(sleep, notified);
                    futures::select_biased! {
                        _ = sleep => {}
                        _ = notified => {}
                    };
                }
            } else if mode == LocalQueueMode::Polling && !self.0.continuous {
                self.set_mode(LocalQueueMode::Released).await;
                break;
            }
        }
    }

    async fn fetch(&self) {
        if !self.try_start_fetch() {
            return;
        }

        if self.is_refetch_delay_active() {
            self.set_refetch_delay_fetch_on_complete(true);
            self.end_fetch();
            self.0.state_notify.notify_one();
            return;
        }

        self.set_fetch_again(false);
        self.reset_refetch_delay_counter();

        let task_details = self.0.task_details.read().await;
        let now = self.0.use_local_time.then(Utc::now);
        let result = batch_get_jobs(
            &self.0.database,
            &task_details,
            &self.0.escaped_schema,
            &self.0.worker_id,
            &[],
            self.0.config.size.try_into().unwrap_or(i32::MAX),
            now,
        )
        .await;
        drop(task_details);

        self.end_fetch();
        self.0.state_notify.notify_one();

        match result {
            Ok(jobs) => {
                let job_count = jobs.len();
                debug!(job_count, "LocalQueue fetched jobs from database");

                self.0
                    .hooks
                    .emit(LocalQueueGetJobsCompleteContext {
                        worker_id: self.0.worker_id.clone(),
                        jobs_count: job_count,
                    })
                    .await;

                let fetched_max = job_count >= self.0.config.size;

                if let Some(ref refetch_delay_config) = self.0.config.refetch_delay {
                    let threshold_surpassed =
                        fetched_max || job_count > refetch_delay_config.threshold;

                    if !threshold_surpassed {
                        self.start_refetch_delay(refetch_delay_config).await;
                    }
                }

                if !jobs.is_empty() {
                    self.received_jobs(jobs, fetched_max).await;
                } else if !self.0.continuous {
                    self.set_mode(LocalQueueMode::Released).await;
                }
            }
            Err(e) => {
                error!(error = %e, "LocalQueue failed to fetch jobs");
            }
        }
    }

    async fn start_refetch_delay(&self, config: &RefetchDelayConfig) {
        let max_abort = config.max_abort_threshold.unwrap_or(5 * self.0.config.size);
        let abort_threshold = if max_abort == usize::MAX {
            usize::MAX
        } else {
            let random: f64 = rand::rng().random();
            ((random * max_abort as f64) as usize).max(1)
        };

        *self.0.refetch_delay.abort_threshold.write().await = abort_threshold;
        self.set_refetch_delay_active(true);
        self.set_refetch_delay_fetch_on_complete(false);

        let duration = Duration::from_millis(
            ((0.5 + rand::rng().random::<f64>() * 0.5) * config.duration.as_millis() as f64) as u64,
        );

        trace!(
            ?duration,
            abort_threshold,
            "LocalQueue starting refetch delay"
        );

        self.0
            .hooks
            .emit(LocalQueueRefetchDelayStartContext {
                worker_id: self.0.worker_id.clone(),
                duration,
                threshold: config.threshold,
                abort_threshold,
            })
            .await;

        let queue_clone = self.clone();
        let refetch_delay_task = runtime::spawn(async move {
            let sleep = runtime::sleep(duration).fuse();
            let notified = queue_clone.0.refetch_delay.abort_notify.notified().fuse();
            futures::pin_mut!(sleep, notified);
            futures::select_biased! {
                _ = sleep => {
                    queue_clone.refetch_delay_complete(false).await;
                }
                _ = notified => {
                    queue_clone.refetch_delay_complete(true).await;
                }
            };
        });
        self.0.refetch_delay_task.replace_abort(refetch_delay_task);
    }

    async fn refetch_delay_complete(&self, aborted: bool) {
        self.set_refetch_delay_active(false);
        self.0.state_notify.notify_one();

        if aborted {
            let count = self.get_refetch_delay_counter();
            let abort_threshold = *self.0.refetch_delay.abort_threshold.read().await;

            self.set_refetch_delay_fetch_on_complete(true);
            trace!("LocalQueue refetch delay aborted");

            self.0
                .hooks
                .emit(LocalQueueRefetchDelayAbortContext {
                    worker_id: self.0.worker_id.clone(),
                    count,
                    abort_threshold,
                })
                .await;
        } else {
            trace!("LocalQueue refetch delay expired");

            self.0
                .hooks
                .emit(LocalQueueRefetchDelayExpiredContext {
                    worker_id: self.0.worker_id.clone(),
                })
                .await;
        }
    }

    async fn check_refetch_delay_abort(&self) {
        if !self.is_refetch_delay_active() {
            return;
        }

        let counter = self.get_refetch_delay_counter();
        let abort_threshold = *self.0.refetch_delay.abort_threshold.read().await;

        if counter >= abort_threshold {
            self.0.refetch_delay.abort_notify.notify_one();
        }
    }

    async fn received_jobs(&self, jobs: Vec<Job>, fetched_max: bool) {
        let job_count = jobs.len();
        {
            let mut job_queue = self.0.job_queue.lock().await;
            job_queue.extend(jobs);
        }

        self.set_mode(LocalQueueMode::Waiting).await;
        self.start_ttl_timer().await;

        trace!(job_count, "Jobs added to cache, signaling stream");

        let _ = self.0.job_signal_sender.try_send(());

        if fetched_max {
            self.set_fetch_again(true);
        }
    }

    async fn start_ttl_timer(&self) {
        let ttl = self.0.config.ttl;

        let queue_clone = self.clone();
        let ttl_timer_task = runtime::spawn(async move {
            runtime::sleep(ttl).await;

            let mode = *queue_clone.0.mode.read().await;
            if mode == LocalQueueMode::Waiting {
                queue_clone.set_mode_ttl_expired().await;
            }
        });

        self.0.ttl_timer_task.replace_abort(ttl_timer_task);
    }

    async fn return_jobs_with_retry(
        database: &Database,
        jobs: &[Job],
        escaped_schema: &str,
        worker_id: &str,
    ) -> Result<(), LocalQueueError> {
        let mut attempt = 0u32;
        loop {
            match return_jobs(database, jobs, escaped_schema, worker_id).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempt += 1;
                    if attempt >= RETURN_JOBS_RETRY_OPTIONS.max_attempts {
                        return Err(LocalQueueError::ReturnJobsError(format!(
                            "Failed after {} attempts: {}",
                            attempt, e
                        )));
                    }
                    let delay = calculate_retry_delay(attempt - 1, &RETURN_JOBS_RETRY_OPTIONS);
                    warn!(
                        attempt,
                        max_attempts = RETURN_JOBS_RETRY_OPTIONS.max_attempts,
                        ?delay,
                        error = %e,
                        "Failed to return jobs, retrying"
                    );
                    runtime::sleep(delay).await;
                }
            }
        }
    }

    async fn set_mode_ttl_expired(&self) {
        let mut mode = self.0.mode.write().await;
        if *mode != LocalQueueMode::Waiting {
            return;
        }
        *mode = LocalQueueMode::TtlExpired;
        drop(mode);

        debug!("LocalQueue TTL expired, returning jobs to database");

        let jobs: Vec<Job> = self.0.job_queue.lock().await.drain(..).collect();
        if !jobs.is_empty() {
            let jobs_count = jobs.len();
            if let Err(e) = Self::return_jobs_with_retry(
                &self.0.database,
                &jobs,
                &self.0.escaped_schema,
                &self.0.worker_id,
            )
            .await
            {
                error!(error = %e, "Failed to return jobs after TTL expiry (exhausted retries)");
            } else {
                self.0
                    .hooks
                    .emit(LocalQueueReturnJobsContext {
                        worker_id: self.0.worker_id.clone(),
                        jobs_count,
                    })
                    .await;
            }
        }
    }

    pub async fn get_job(&self, flags_to_skip: &[String]) -> Option<Job> {
        let mode = *self.0.mode.read().await;
        if mode == LocalQueueMode::Released {
            return None;
        }

        if !flags_to_skip.is_empty() {
            return self.get_job_direct(flags_to_skip).await;
        }

        self.get_job_from_cache().await
    }

    async fn get_job_direct(&self, flags_to_skip: &[String]) -> Option<Job> {
        let task_details = self.0.task_details.read().await;
        let now = self.0.use_local_time.then(Utc::now);
        match get_job(
            &self.0.database,
            &task_details,
            &self.0.escaped_schema,
            &self.0.worker_id,
            flags_to_skip,
            now,
        )
        .await
        {
            Ok(job) => job,
            Err(e) => {
                error!(error = %e, "LocalQueue direct get_job failed");
                None
            }
        }
    }

    async fn get_job_from_cache(&self) -> Option<Job> {
        {
            let mode = *self.0.mode.read().await;
            if mode == LocalQueueMode::TtlExpired {
                self.set_mode(LocalQueueMode::Polling).await;
            }
            if mode == LocalQueueMode::Released {
                return None;
            }
        }

        let mut job_queue = self.0.job_queue.lock().await;
        if let Some(job) = job_queue.pop_front() {
            let remaining = job_queue.len();
            drop(job_queue);

            if remaining == 0 {
                self.set_mode(LocalQueueMode::Polling).await;
            }

            return Some(job);
        }

        None
    }

    pub async fn pulse(&self, count: usize) {
        trace!(count, "LocalQueue received pulse");

        self.increment_refetch_delay_counter(count);
        self.check_refetch_delay_abort().await;

        let mode = *self.0.mode.read().await;

        match mode {
            LocalQueueMode::Polling => {
                if self.is_fetch_in_progress() {
                    self.set_fetch_again(true);
                    self.0.state_notify.notify_one();
                } else if !self.is_refetch_delay_active() {
                    self.0.state_notify.notify_one();
                }
            }
            LocalQueueMode::Waiting | LocalQueueMode::TtlExpired => {}
            LocalQueueMode::Released | LocalQueueMode::Starting => {}
        }
    }

    pub async fn release(&self) -> Result<(), LocalQueueError> {
        let mut mode = self.0.mode.write().await;
        if *mode == LocalQueueMode::Released {
            return Ok(());
        }
        *mode = LocalQueueMode::Released;
        drop(mode);

        self.set_refetch_delay_active(false);
        self.0.refetch_delay.abort_notify.notify_waiters();
        self.0.state_notify.notify_waiters();

        self.0.ttl_timer_task.abort();
        self.0.refetch_delay_task.abort();

        debug!("LocalQueue releasing, returning jobs to database");

        let jobs: Vec<Job> = self.0.job_queue.lock().await.drain(..).collect();
        if !jobs.is_empty() {
            let jobs_count = jobs.len();
            Self::return_jobs_with_retry(
                &self.0.database,
                &jobs,
                &self.0.escaped_schema,
                &self.0.worker_id,
            )
            .await?;

            self.0
                .hooks
                .emit(LocalQueueReturnJobsContext {
                    worker_id: self.0.worker_id.clone(),
                    jobs_count,
                })
                .await;
        }

        self.0.run_complete_notify.notified().await;

        Ok(())
    }

    async fn set_mode(&self, new_mode: LocalQueueMode) {
        let mut mode = self.0.mode.write().await;
        let old_mode = *mode;
        if old_mode == new_mode {
            return;
        }
        trace!(?old_mode, ?new_mode, "LocalQueue mode transition");
        *mode = new_mode;
        drop(mode);

        self.0
            .hooks
            .emit(LocalQueueSetModeContext {
                worker_id: self.0.worker_id.clone(),
                old_mode,
                new_mode,
            })
            .await;

        self.0.state_notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::config::RetryOptions;
    use super::*;

    #[test]
    fn retry_delay_applies_cap_and_jitter() {
        let options = RetryOptions {
            max_attempts: 3,
            min_delay_ms: 1_000,
            max_delay_ms: 10,
            multiplier: 2.0,
        };

        let delay = calculate_retry_delay(4, &options);
        assert!(delay >= Duration::from_millis(5));
        assert!(delay <= Duration::from_millis(10));
    }

    #[test]
    fn local_queue_config_builders_and_mutators() {
        let refetch_delay = RefetchDelayConfig::default()
            .with_duration(Duration::from_millis(50))
            .with_threshold(2)
            .with_max_abort_threshold(9);

        assert_eq!(refetch_delay.duration, Duration::from_millis(50));
        assert_eq!(refetch_delay.threshold, 2);
        assert_eq!(refetch_delay.max_abort_threshold, Some(9));

        let built_refetch_delay = RefetchDelayConfig::builder()
            .duration(Duration::from_millis(25))
            .threshold(1)
            .max_abort_threshold(4)
            .build();
        assert_eq!(built_refetch_delay.duration, Duration::from_millis(25));
        assert_eq!(built_refetch_delay.threshold, 1);
        assert_eq!(built_refetch_delay.max_abort_threshold, Some(4));

        let config = LocalQueueConfig::default()
            .with_size(7)
            .with_ttl(Duration::from_secs(3))
            .with_refetch_delay(refetch_delay.clone())
            .with_queue_count(3);
        assert_eq!(config.size, 7);
        assert_eq!(config.ttl, Duration::from_secs(3));
        assert!(config.refetch_delay.is_some());
        assert_eq!(config.queue_count, 3);

        let built_config = LocalQueueConfig::builder()
            .size(8)
            .ttl(Duration::from_secs(4))
            .refetch_delay(built_refetch_delay)
            .queue_count(2)
            .build();
        assert_eq!(built_config.size, 8);
        assert_eq!(built_config.ttl, Duration::from_secs(4));
        assert!(built_config.refetch_delay.is_some());
        assert_eq!(built_config.queue_count, 2);
    }
}
