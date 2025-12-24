use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker_job::Job;
pub use graphile_worker_lifecycle_hooks::LocalQueueMode;
use graphile_worker_lifecycle_hooks::{
    HookRegistry, LocalQueueGetJobsCompleteContext, LocalQueueInitContext,
    LocalQueueRefetchDelayAbortContext, LocalQueueRefetchDelayExpiredContext,
    LocalQueueRefetchDelayStartContext, LocalQueueReturnJobsContext, LocalQueueSetModeContext,
};
use graphile_worker_shutdown_signal::ShutdownSignal;
use rand::Rng;
use sqlx::PgPool;
use thiserror::Error;
use tokio::sync::{Mutex, Notify, RwLock};

use crate::streams::JobSignalSender;
use tokio::task::AbortHandle;
use tracing::{debug, error, trace, warn};

use crate::sql::batch_get_jobs::batch_get_jobs;
use crate::sql::get_job::get_job;
use crate::sql::return_jobs::return_jobs;
use crate::sql::task_identifiers::TaskDetails;

const DEFAULT_LOCAL_QUEUE_SIZE: usize = 100;
const DEFAULT_LOCAL_QUEUE_TTL: Duration = Duration::from_secs(5 * 60);

struct RetryOptions {
    max_attempts: u32,
    min_delay_ms: u64,
    max_delay_ms: u64,
    multiplier: f64,
}

const RETURN_JOBS_RETRY_OPTIONS: RetryOptions = RetryOptions {
    max_attempts: 20,
    min_delay_ms: 200,
    max_delay_ms: 30_000,
    multiplier: 1.5,
};

fn calculate_retry_delay(attempt: u32, options: &RetryOptions) -> Duration {
    let base = options.min_delay_ms as f64 * options.multiplier.powi(attempt as i32);
    let capped = base.min(options.max_delay_ms as f64);
    let jittered = capped * (0.5 + rand::rng().random::<f64>() * 0.5);
    Duration::from_millis(jittered as u64)
}

#[derive(Debug, Clone)]
pub struct RefetchDelayConfig {
    pub duration: Duration,
    pub threshold: usize,
    pub max_abort_threshold: Option<usize>,
}

impl Default for RefetchDelayConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_millis(100),
            threshold: 0,
            max_abort_threshold: None,
        }
    }
}

impl RefetchDelayConfig {
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }

    pub fn with_max_abort_threshold(mut self, max_abort_threshold: usize) -> Self {
        self.max_abort_threshold = Some(max_abort_threshold);
        self
    }
}

#[derive(Debug, Clone)]
pub struct LocalQueueConfig {
    pub size: usize,
    pub ttl: Duration,
    pub refetch_delay: Option<RefetchDelayConfig>,
}

impl Default for LocalQueueConfig {
    fn default() -> Self {
        Self {
            size: DEFAULT_LOCAL_QUEUE_SIZE,
            ttl: DEFAULT_LOCAL_QUEUE_TTL,
            refetch_delay: None,
        }
    }
}

impl LocalQueueConfig {
    pub fn with_size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn with_refetch_delay(mut self, refetch_delay: RefetchDelayConfig) -> Self {
        self.refetch_delay = Some(refetch_delay);
        self
    }
}

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
    abort_threshold: RwLock<usize>,
    abort_notify: Notify,
}

impl Default for RefetchDelayState {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(false),
            fetch_on_complete: AtomicBool::new(false),
            counter: AtomicUsize::new(0),
            abort_threshold: RwLock::new(usize::MAX),
            abort_notify: Notify::new(),
        }
    }
}

struct LocalQueueState {
    mode: RwLock<LocalQueueMode>,
    job_queue: Mutex<VecDeque<Job>>,
    job_signal_sender: JobSignalSender,
    fetch_in_progress: AtomicBool,
    fetch_again: AtomicBool,
    refetch_delay: RefetchDelayState,
    state_notify: Notify,
    ttl_timer_handle: Mutex<Option<AbortHandle>>,
    run_complete_notify: Notify,
    config: LocalQueueConfig,
    pg_pool: PgPool,
    escaped_schema: String,
    worker_id: String,
    task_details: Arc<RwLock<TaskDetails>>,
    poll_interval: Duration,
    continuous: bool,
    hooks: Arc<HookRegistry>,
}

impl LocalQueueState {
    fn new(params: LocalQueueParams) -> Self {
        Self {
            mode: RwLock::new(LocalQueueMode::Starting),
            job_queue: Mutex::new(VecDeque::new()),
            job_signal_sender: params.job_signal_sender,
            fetch_in_progress: AtomicBool::new(false),
            fetch_again: AtomicBool::new(false),
            refetch_delay: RefetchDelayState::default(),
            state_notify: Notify::new(),
            ttl_timer_handle: Mutex::new(None),
            run_complete_notify: Notify::new(),
            config: params.config,
            pg_pool: params.pg_pool,
            escaped_schema: params.escaped_schema,
            worker_id: params.worker_id,
            task_details: params.task_details,
            poll_interval: params.poll_interval,
            continuous: params.continuous,
            hooks: params.hooks,
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

pub struct LocalQueueParams {
    pub config: LocalQueueConfig,
    pub pg_pool: PgPool,
    pub escaped_schema: String,
    pub worker_id: String,
    pub task_details: Arc<RwLock<TaskDetails>>,
    pub poll_interval: Duration,
    pub continuous: bool,
    pub shutdown_signal: Option<ShutdownSignal>,
    pub hooks: Arc<HookRegistry>,
    pub job_signal_sender: JobSignalSender,
}

impl LocalQueue {
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

        if params.config.size > i32::MAX as usize {
            panic!(
                "local_queue.size ({}) must not exceed i32::MAX ({})",
                params.config.size,
                i32::MAX
            );
        }

        let shutdown_signal = params.shutdown_signal.clone();
        let queue: LocalQueue = LocalQueueState::new(params).into();

        let queue_clone = queue.clone();
        tokio::spawn(async move {
            queue_clone.run().await;
        });

        if let Some(signal) = shutdown_signal {
            let queue_for_shutdown = queue.clone();
            tokio::spawn(async move {
                signal.await;
                if let Err(e) = queue_for_shutdown.release().await {
                    warn!(error = %e, "Error releasing LocalQueue on shutdown");
                }
            });
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
                && !self.0.fetch_in_progress.load(Ordering::SeqCst)
                && !self.0.refetch_delay.active.load(Ordering::SeqCst);

            if !can_fetch {
                self.0.state_notify.notified().await;
                continue;
            }

            self.fetch().await;

            let mode = *self.0.mode.read().await;
            if mode == LocalQueueMode::Polling && self.0.continuous {
                let should_fetch_again = self.0.fetch_again.swap(false, Ordering::SeqCst);
                let refetch_delay_wants_fetch = self
                    .0
                    .refetch_delay
                    .fetch_on_complete
                    .swap(false, Ordering::SeqCst);

                if !should_fetch_again && !refetch_delay_wants_fetch {
                    tokio::select! {
                        _ = tokio::time::sleep(self.0.poll_interval) => {}
                        _ = self.0.state_notify.notified() => {}
                    }
                }
            } else if mode == LocalQueueMode::Polling && !self.0.continuous {
                self.set_mode(LocalQueueMode::Released).await;
                break;
            }
        }
    }

    async fn fetch(&self) {
        if self
            .0
            .fetch_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        if self.0.refetch_delay.active.load(Ordering::SeqCst) {
            self.0
                .refetch_delay
                .fetch_on_complete
                .store(true, Ordering::SeqCst);
            self.0.fetch_in_progress.store(false, Ordering::SeqCst);
            self.0.state_notify.notify_one();
            return;
        }

        self.0.fetch_again.store(false, Ordering::SeqCst);
        self.0.refetch_delay.counter.store(0, Ordering::SeqCst);

        let task_details = self.0.task_details.read().await;
        let result = batch_get_jobs(
            &self.0.pg_pool,
            &task_details,
            &self.0.escaped_schema,
            &self.0.worker_id,
            &[],
            self.0.config.size.try_into().unwrap_or(i32::MAX),
        )
        .await;
        drop(task_details);

        self.0.fetch_in_progress.store(false, Ordering::SeqCst);
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
        self.0.refetch_delay.active.store(true, Ordering::SeqCst);
        self.0
            .refetch_delay
            .fetch_on_complete
            .store(false, Ordering::SeqCst);

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
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(duration) => {
                    queue_clone.refetch_delay_complete(false).await;
                }
                _ = queue_clone.0.refetch_delay.abort_notify.notified() => {
                    queue_clone.refetch_delay_complete(true).await;
                }
            }
        });
    }

    async fn refetch_delay_complete(&self, aborted: bool) {
        self.0.refetch_delay.active.store(false, Ordering::SeqCst);
        self.0.state_notify.notify_one();

        if aborted {
            let count = self.0.refetch_delay.counter.load(Ordering::SeqCst);
            let abort_threshold = *self.0.refetch_delay.abort_threshold.read().await;

            self.0
                .refetch_delay
                .fetch_on_complete
                .store(true, Ordering::SeqCst);
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
        if !self.0.refetch_delay.active.load(Ordering::SeqCst) {
            return;
        }

        let counter = self.0.refetch_delay.counter.load(Ordering::SeqCst);
        let abort_threshold = *self.0.refetch_delay.abort_threshold.read().await;

        if counter >= abort_threshold {
            self.0.refetch_delay.abort_notify.notify_one();
        }
    }

    async fn received_jobs(&self, jobs: Vec<Job>, fetched_max: bool) {
        let job_count = jobs.len();
        {
            let mut job_queue = self.0.job_queue.lock().await;
            for job in jobs {
                job_queue.push_back(job);
            }
        }

        self.set_mode(LocalQueueMode::Waiting).await;
        self.start_ttl_timer().await;

        trace!(job_count, "Jobs added to cache, signaling stream");

        let _ = self.0.job_signal_sender.try_send(());

        if fetched_max || self.0.fetch_again.load(Ordering::SeqCst) {
            self.0.fetch_again.store(true, Ordering::SeqCst);
        }
    }

    async fn start_ttl_timer(&self) {
        let ttl = self.0.config.ttl;

        let mut handle_guard = self.0.ttl_timer_handle.lock().await;
        if let Some(existing_handle) = handle_guard.take() {
            existing_handle.abort();
        }

        let queue_clone = self.clone();
        let join_handle = tokio::spawn(async move {
            tokio::time::sleep(ttl).await;

            let mode = *queue_clone.0.mode.read().await;
            if mode == LocalQueueMode::Waiting {
                queue_clone.set_mode_ttl_expired().await;
            }
        });

        *handle_guard = Some(join_handle.abort_handle());
    }

    async fn return_jobs_with_retry(
        pool: &PgPool,
        jobs: &[Job],
        escaped_schema: &str,
        worker_id: &str,
    ) -> Result<(), LocalQueueError> {
        let mut attempt = 0u32;
        loop {
            match return_jobs(pool, jobs, escaped_schema, worker_id).await {
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
                    tokio::time::sleep(delay).await;
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
                &self.0.pg_pool,
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
        match get_job(
            &self.0.pg_pool,
            &task_details,
            &self.0.escaped_schema,
            &self.0.worker_id,
            &flags_to_skip.to_vec(),
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
            } else {
                let _ = self.0.job_signal_sender.try_send(());
            }

            return Some(job);
        }

        None
    }

    pub async fn pulse(&self, count: usize) {
        trace!(count, "LocalQueue received pulse");

        self.0
            .refetch_delay
            .counter
            .fetch_add(count, Ordering::SeqCst);
        self.check_refetch_delay_abort().await;

        let mode = *self.0.mode.read().await;

        match mode {
            LocalQueueMode::Polling => {
                if self.0.fetch_in_progress.load(Ordering::SeqCst) {
                    self.0.fetch_again.store(true, Ordering::SeqCst);
                    self.0.state_notify.notify_one();
                } else if !self.0.refetch_delay.active.load(Ordering::SeqCst) {
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

        self.0.refetch_delay.active.store(false, Ordering::SeqCst);
        self.0.refetch_delay.abort_notify.notify_waiters();
        self.0.state_notify.notify_waiters();

        if let Some(handle) = self.0.ttl_timer_handle.lock().await.take() {
            handle.abort();
        }

        debug!("LocalQueue releasing, returning jobs to database");

        let jobs: Vec<Job> = self.0.job_queue.lock().await.drain(..).collect();
        if !jobs.is_empty() {
            let jobs_count = jobs.len();
            Self::return_jobs_with_retry(
                &self.0.pg_pool,
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
