use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use graphile_worker_job::Job;
pub use graphile_worker_lifecycle_hooks::LocalQueueMode;
use graphile_worker_lifecycle_hooks::{
    LocalQueueGetJobsCompleteContext, LocalQueueInitContext, LocalQueueRefetchDelayAbortContext,
    LocalQueueRefetchDelayExpiredContext, LocalQueueRefetchDelayStartContext,
    LocalQueueReturnJobsContext, LocalQueueSetModeContext, TypeErasedHooks,
};
use graphile_worker_shutdown_signal::ShutdownSignal;
use rand::Rng;
use sqlx::PgPool;
use thiserror::Error;
use tokio::sync::{oneshot, Mutex, Notify, RwLock};
use tokio::task::AbortHandle;
use tracing::{debug, error, trace, warn};

use crate::sql::batch_get_jobs::batch_get_jobs;
use crate::sql::get_job::get_job;
use crate::sql::return_jobs::return_jobs;
use crate::sql::task_identifiers::TaskDetails;

const DEFAULT_LOCAL_QUEUE_SIZE: usize = 100;
const DEFAULT_LOCAL_QUEUE_TTL: Duration = Duration::from_secs(5 * 60);

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

struct LocalQueueInner {
    mode: RwLock<LocalQueueMode>,
    job_queue: Mutex<VecDeque<Job>>,
    worker_queue: Mutex<VecDeque<oneshot::Sender<Option<Job>>>>,
    fetch_in_progress: AtomicBool,
    fetch_again: AtomicBool,
    refetch_delay: RefetchDelayState,
    state_notify: Notify,
    ttl_timer_handle: Mutex<Option<AbortHandle>>,
}

pub struct LocalQueue {
    inner: LocalQueueInner,
    config: LocalQueueConfig,
    pg_pool: PgPool,
    escaped_schema: String,
    worker_id: String,
    task_details: Arc<RwLock<TaskDetails>>,
    poll_interval: Duration,
    continuous: bool,
    hooks: Arc<TypeErasedHooks>,
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
    pub hooks: Arc<TypeErasedHooks>,
}

impl LocalQueue {
    pub fn new(params: LocalQueueParams) -> Arc<Self> {
        let LocalQueueParams {
            config,
            pg_pool,
            escaped_schema,
            worker_id,
            task_details,
            poll_interval,
            continuous,
            shutdown_signal,
            hooks,
        } = params;
        if let Some(ref refetch_delay) = config.refetch_delay {
            if refetch_delay.duration > poll_interval {
                panic!(
                    "refetch_delay.duration ({:?}) must not be larger than poll_interval ({:?})",
                    refetch_delay.duration, poll_interval
                );
            }
        }

        if config.size == 0 {
            panic!("local_queue.size must be greater than 0");
        }

        if config.size > i32::MAX as usize {
            panic!(
                "local_queue.size ({}) must not exceed i32::MAX ({})",
                config.size,
                i32::MAX
            );
        }

        let queue = Arc::new(Self {
            inner: LocalQueueInner {
                mode: RwLock::new(LocalQueueMode::Starting),
                job_queue: Mutex::new(VecDeque::new()),
                worker_queue: Mutex::new(VecDeque::new()),
                fetch_in_progress: AtomicBool::new(false),
                fetch_again: AtomicBool::new(false),
                refetch_delay: RefetchDelayState::default(),
                state_notify: Notify::new(),
                ttl_timer_handle: Mutex::new(None),
            },
            config,
            pg_pool,
            escaped_schema,
            worker_id: worker_id.clone(),
            task_details,
            poll_interval,
            continuous,
            hooks: hooks.clone(),
        });

        let queue_clone = Arc::clone(&queue);
        tokio::spawn(async move {
            Self::run(queue_clone).await;
        });

        if let Some(signal) = shutdown_signal {
            let queue_for_shutdown = Arc::clone(&queue);
            tokio::spawn(async move {
                signal.await;
                if let Err(e) = queue_for_shutdown.release().await {
                    warn!(error = %e, "Error releasing LocalQueue on shutdown");
                }
            });
        }

        queue
    }

    async fn run(this: Arc<Self>) {
        this.hooks
            .emit_local_queue_init(LocalQueueInitContext {
                worker_id: this.worker_id.clone(),
            })
            .await;

        Self::set_mode(&this, LocalQueueMode::Polling).await;
        Self::schedule_fetch(this).await;
    }

    async fn schedule_fetch(this: Arc<Self>) {
        loop {
            let mode = *this.inner.mode.read().await;

            if mode == LocalQueueMode::Released {
                break;
            }

            let can_fetch = mode == LocalQueueMode::Polling
                && !this.inner.fetch_in_progress.load(Ordering::SeqCst)
                && !this.inner.refetch_delay.active.load(Ordering::SeqCst);

            if !can_fetch {
                this.inner.state_notify.notified().await;
                continue;
            }

            Self::fetch(Arc::clone(&this)).await;

            let mode = *this.inner.mode.read().await;
            if mode == LocalQueueMode::Polling && this.continuous {
                let should_fetch_again = this.inner.fetch_again.swap(false, Ordering::SeqCst);
                let refetch_delay_wants_fetch = this
                    .inner
                    .refetch_delay
                    .fetch_on_complete
                    .swap(false, Ordering::SeqCst);

                if !should_fetch_again && !refetch_delay_wants_fetch {
                    tokio::select! {
                        _ = tokio::time::sleep(this.poll_interval) => {}
                        _ = this.inner.state_notify.notified() => {}
                    }
                }
            } else if mode == LocalQueueMode::Polling && !this.continuous {
                Self::set_mode(&this, LocalQueueMode::Released).await;
                break;
            }
        }
    }

    async fn fetch(this: Arc<Self>) {
        if this
            .inner
            .fetch_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        if this.inner.refetch_delay.active.load(Ordering::SeqCst) {
            this.inner
                .refetch_delay
                .fetch_on_complete
                .store(true, Ordering::SeqCst);
            this.inner.fetch_in_progress.store(false, Ordering::SeqCst);
            this.inner.state_notify.notify_one();
            return;
        }

        this.inner.fetch_again.store(false, Ordering::SeqCst);
        this.inner.refetch_delay.counter.store(0, Ordering::SeqCst);

        let task_details = this.task_details.read().await;
        let result = batch_get_jobs(
            &this.pg_pool,
            &task_details,
            &this.escaped_schema,
            &this.worker_id,
            &[],
            this.config.size.try_into().unwrap_or(i32::MAX),
        )
        .await;
        drop(task_details);

        this.inner.fetch_in_progress.store(false, Ordering::SeqCst);
        this.inner.state_notify.notify_one();

        match result {
            Ok(jobs) => {
                let job_count = jobs.len();
                debug!(job_count, "LocalQueue fetched jobs from database");

                this.hooks
                    .emit_local_queue_get_jobs_complete(LocalQueueGetJobsCompleteContext {
                        worker_id: this.worker_id.clone(),
                        jobs_count: job_count,
                    })
                    .await;

                let fetched_max = job_count >= this.config.size;

                if let Some(ref refetch_delay_config) = this.config.refetch_delay {
                    let threshold_surpassed =
                        fetched_max || job_count > refetch_delay_config.threshold;

                    if !threshold_surpassed {
                        Self::start_refetch_delay(Arc::clone(&this), refetch_delay_config).await;
                    }
                }

                if !jobs.is_empty() {
                    Self::received_jobs(Arc::clone(&this), jobs, fetched_max).await;
                } else if !this.continuous {
                    Self::set_mode(&this, LocalQueueMode::Released).await;
                }
            }
            Err(e) => {
                error!(error = %e, "LocalQueue failed to fetch jobs");
            }
        }
    }

    async fn start_refetch_delay(this: Arc<Self>, config: &RefetchDelayConfig) {
        let max_abort = config.max_abort_threshold.unwrap_or(5 * this.config.size);
        let abort_threshold = if max_abort == usize::MAX {
            usize::MAX
        } else {
            let random: f64 = rand::rng().random();
            ((random * max_abort as f64) as usize).max(1)
        };

        *this.inner.refetch_delay.abort_threshold.write().await = abort_threshold;
        this.inner
            .refetch_delay
            .active
            .store(true, Ordering::SeqCst);
        this.inner
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

        this.hooks
            .emit_local_queue_refetch_delay_start(LocalQueueRefetchDelayStartContext {
                worker_id: this.worker_id.clone(),
                duration,
                threshold: config.threshold,
                abort_threshold,
            })
            .await;

        let this_clone = Arc::clone(&this);
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(duration) => {
                    Self::refetch_delay_complete(this_clone, false).await;
                }
                _ = this_clone.inner.refetch_delay.abort_notify.notified() => {
                    Self::refetch_delay_complete(this_clone, true).await;
                }
            }
        });
    }

    async fn refetch_delay_complete(this: Arc<Self>, aborted: bool) {
        this.inner
            .refetch_delay
            .active
            .store(false, Ordering::SeqCst);
        this.inner.state_notify.notify_one();

        if aborted {
            let count = this.inner.refetch_delay.counter.load(Ordering::SeqCst);
            let abort_threshold = *this.inner.refetch_delay.abort_threshold.read().await;

            this.inner
                .refetch_delay
                .fetch_on_complete
                .store(true, Ordering::SeqCst);
            trace!("LocalQueue refetch delay aborted");

            this.hooks
                .emit_local_queue_refetch_delay_abort(LocalQueueRefetchDelayAbortContext {
                    worker_id: this.worker_id.clone(),
                    count,
                    abort_threshold,
                })
                .await;
        } else {
            trace!("LocalQueue refetch delay expired");

            this.hooks
                .emit_local_queue_refetch_delay_expired(LocalQueueRefetchDelayExpiredContext {
                    worker_id: this.worker_id.clone(),
                })
                .await;
        }
    }

    async fn check_refetch_delay_abort(&self) -> bool {
        if !self.inner.refetch_delay.active.load(Ordering::SeqCst) {
            return false;
        }

        let counter = self.inner.refetch_delay.counter.load(Ordering::SeqCst);
        let abort_threshold = *self.inner.refetch_delay.abort_threshold.read().await;

        if counter >= abort_threshold {
            self.inner.refetch_delay.abort_notify.notify_one();
        }

        true
    }

    async fn received_jobs(this: Arc<Self>, jobs: Vec<Job>, fetched_max: bool) {
        let job_count = jobs.len();
        let mut job_queue = this.inner.job_queue.lock().await;
        let mut worker_queue = this.inner.worker_queue.lock().await;

        let mut jobs_iter = jobs.into_iter();

        while let Some(sender) = worker_queue.pop_front() {
            if let Some(job) = jobs_iter.next() {
                let _ = sender.send(Some(job));
            } else {
                worker_queue.push_front(sender);
                break;
            }
        }

        for job in jobs_iter {
            job_queue.push_back(job);
        }

        drop(worker_queue);

        if !job_queue.is_empty() {
            drop(job_queue);
            Self::set_mode(&this, LocalQueueMode::Waiting).await;
            Self::start_ttl_timer(this).await;
        } else {
            drop(job_queue);
            trace!(job_count, "All fetched jobs distributed to workers");

            if fetched_max || this.inner.fetch_again.load(Ordering::SeqCst) {
                this.inner.fetch_again.store(true, Ordering::SeqCst);
            }
        }
    }

    async fn start_ttl_timer(this: Arc<Self>) {
        let ttl = this.config.ttl;

        let mut handle_guard = this.inner.ttl_timer_handle.lock().await;
        if let Some(existing_handle) = handle_guard.take() {
            existing_handle.abort();
        }

        let this_clone = Arc::clone(&this);
        let join_handle = tokio::spawn(async move {
            tokio::time::sleep(ttl).await;

            let mode = *this_clone.inner.mode.read().await;
            if mode == LocalQueueMode::Waiting {
                Self::set_mode_ttl_expired(this_clone).await;
            }
        });

        *handle_guard = Some(join_handle.abort_handle());
    }

    async fn set_mode_ttl_expired(this: Arc<Self>) {
        let mut mode = this.inner.mode.write().await;
        if *mode != LocalQueueMode::Waiting {
            return;
        }
        *mode = LocalQueueMode::TtlExpired;
        drop(mode);

        debug!("LocalQueue TTL expired, returning jobs to database");

        let jobs: Vec<Job> = this.inner.job_queue.lock().await.drain(..).collect();
        if !jobs.is_empty() {
            let jobs_count = jobs.len();
            if let Err(e) =
                return_jobs(&this.pg_pool, &jobs, &this.escaped_schema, &this.worker_id).await
            {
                error!(error = %e, "Failed to return jobs after TTL expiry");
            } else {
                this.hooks
                    .emit_local_queue_return_jobs(LocalQueueReturnJobsContext {
                        worker_id: this.worker_id.clone(),
                        jobs_count,
                    })
                    .await;
            }
        }
    }

    pub async fn get_job(this: &Arc<Self>, flags_to_skip: &[String]) -> Option<Job> {
        let mode = *this.inner.mode.read().await;
        if mode == LocalQueueMode::Released {
            return None;
        }

        if !flags_to_skip.is_empty() {
            return Self::get_job_direct(this, flags_to_skip).await;
        }

        Self::get_job_from_cache(Arc::clone(this)).await
    }

    async fn get_job_direct(this: &Arc<Self>, flags_to_skip: &[String]) -> Option<Job> {
        let task_details = this.task_details.read().await;
        match get_job(
            &this.pg_pool,
            &task_details,
            &this.escaped_schema,
            &this.worker_id,
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

    async fn get_job_from_cache(this: Arc<Self>) -> Option<Job> {
        {
            let mode = *this.inner.mode.read().await;
            if mode == LocalQueueMode::TtlExpired {
                Self::set_mode(&this, LocalQueueMode::Polling).await;
            }
        }

        {
            let mut job_queue = this.inner.job_queue.lock().await;
            if let Some(job) = job_queue.pop_front() {
                let remaining = job_queue.len();
                drop(job_queue);

                if remaining == 0 {
                    Self::set_mode(&this, LocalQueueMode::Polling).await;
                }

                return Some(job);
            }
        }

        let (tx, rx) = oneshot::channel();
        this.inner.worker_queue.lock().await.push_back(tx);

        let mode = *this.inner.mode.read().await;
        if mode == LocalQueueMode::Released {
            return None;
        }

        rx.await.unwrap_or_default()
    }

    pub async fn pulse(&self, count: usize) {
        trace!(count, "LocalQueue received pulse");

        self.inner
            .refetch_delay
            .counter
            .fetch_add(count, Ordering::SeqCst);
        self.check_refetch_delay_abort().await;

        let mode = *self.inner.mode.read().await;

        match mode {
            LocalQueueMode::Polling => {
                if self.inner.fetch_in_progress.load(Ordering::SeqCst) {
                    self.inner.fetch_again.store(true, Ordering::SeqCst);
                    self.inner.state_notify.notify_one();
                }
            }
            LocalQueueMode::Waiting | LocalQueueMode::TtlExpired => {}
            LocalQueueMode::Released | LocalQueueMode::Starting => {}
        }
    }

    pub async fn release(&self) -> Result<(), LocalQueueError> {
        let mut mode = self.inner.mode.write().await;
        if *mode == LocalQueueMode::Released {
            return Ok(());
        }
        *mode = LocalQueueMode::Released;
        drop(mode);

        self.inner
            .refetch_delay
            .active
            .store(false, Ordering::SeqCst);
        self.inner.refetch_delay.abort_notify.notify_waiters();
        self.inner.state_notify.notify_waiters();

        debug!("LocalQueue releasing, returning jobs to database");

        {
            let mut worker_queue = self.inner.worker_queue.lock().await;
            while let Some(tx) = worker_queue.pop_front() {
                let _ = tx.send(None);
            }
        }

        let jobs: Vec<Job> = self.inner.job_queue.lock().await.drain(..).collect();
        if !jobs.is_empty() {
            let jobs_count = jobs.len();
            return_jobs(&self.pg_pool, &jobs, &self.escaped_schema, &self.worker_id)
                .await
                .map_err(|e| LocalQueueError::ReturnJobsError(e.to_string()))?;

            self.hooks
                .emit_local_queue_return_jobs(LocalQueueReturnJobsContext {
                    worker_id: self.worker_id.clone(),
                    jobs_count,
                })
                .await;
        }

        Ok(())
    }

    async fn set_mode(this: &Arc<Self>, new_mode: LocalQueueMode) {
        let mut mode = this.inner.mode.write().await;
        let old_mode = *mode;
        if old_mode == new_mode {
            return;
        }
        trace!(?old_mode, ?new_mode, "LocalQueue mode transition");
        *mode = new_mode;
        drop(mode);

        this.hooks
            .emit_local_queue_set_mode(LocalQueueSetModeContext {
                worker_id: this.worker_id.clone(),
                old_mode,
                new_mode,
            })
            .await;

        this.inner.state_notify.notify_waiters();
    }
}
