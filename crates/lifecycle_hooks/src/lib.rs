mod context;
mod result;
mod traits;

use std::sync::Arc;

use futures::future::BoxFuture;

pub use context::*;
pub use result::*;
pub use traits::LifecycleHooks;

pub type ObserverFn<Ctx> = Box<dyn Fn(Ctx) -> BoxFuture<'static, ()> + Send + Sync>;
pub type InterceptorFn<Ctx> = Box<dyn Fn(Ctx) -> BoxFuture<'static, HookResult> + Send + Sync>;
pub type ScheduleTransformerFn =
    Box<dyn Fn(BeforeJobScheduleContext) -> BoxFuture<'static, JobScheduleResult> + Send + Sync>;

#[derive(Default)]
pub struct TypeErasedHooks {
    pub on_worker_init: Vec<ObserverFn<WorkerInitContext>>,
    pub on_worker_start: Vec<ObserverFn<WorkerStartContext>>,
    pub on_worker_shutdown: Vec<ObserverFn<WorkerShutdownContext>>,
    pub on_job_fetch: Vec<ObserverFn<JobFetchContext>>,
    pub on_job_start: Vec<ObserverFn<JobStartContext>>,
    pub on_job_complete: Vec<ObserverFn<JobCompleteContext>>,
    pub on_job_fail: Vec<ObserverFn<JobFailContext>>,
    pub on_job_permanently_fail: Vec<ObserverFn<JobPermanentlyFailContext>>,
    pub on_cron_tick: Vec<ObserverFn<CronTickContext>>,
    pub on_cron_job_scheduled: Vec<ObserverFn<CronJobScheduledContext>>,
    pub on_local_queue_init: Vec<ObserverFn<LocalQueueInitContext>>,
    pub on_local_queue_set_mode: Vec<ObserverFn<LocalQueueSetModeContext>>,
    pub on_local_queue_get_jobs_complete: Vec<ObserverFn<LocalQueueGetJobsCompleteContext>>,
    pub on_local_queue_return_jobs: Vec<ObserverFn<LocalQueueReturnJobsContext>>,
    pub on_local_queue_refetch_delay_start: Vec<ObserverFn<LocalQueueRefetchDelayStartContext>>,
    pub on_local_queue_refetch_delay_abort: Vec<ObserverFn<LocalQueueRefetchDelayAbortContext>>,
    pub on_local_queue_refetch_delay_expired: Vec<ObserverFn<LocalQueueRefetchDelayExpiredContext>>,
    pub before_job_run: Vec<InterceptorFn<BeforeJobRunContext>>,
    pub after_job_run: Vec<InterceptorFn<AfterJobRunContext>>,
    pub before_job_schedule: Vec<ScheduleTransformerFn>,
}

impl TypeErasedHooks {
    pub fn register<H: LifecycleHooks>(&mut self, hook: H) {
        let hook = Arc::new(hook);

        let h = hook.clone();
        self.on_worker_init.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_worker_init(ctx).await })
        }));

        let h = hook.clone();
        self.on_worker_start.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_worker_start(ctx).await })
        }));

        let h = hook.clone();
        self.on_worker_shutdown.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_worker_shutdown(ctx).await })
        }));

        let h = hook.clone();
        self.on_job_fetch.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_job_fetch(ctx).await })
        }));

        let h = hook.clone();
        self.on_job_start.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_job_start(ctx).await })
        }));

        let h = hook.clone();
        self.on_job_complete.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_job_complete(ctx).await })
        }));

        let h = hook.clone();
        self.on_job_fail.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_job_fail(ctx).await })
        }));

        let h = hook.clone();
        self.on_job_permanently_fail.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_job_permanently_fail(ctx).await })
        }));

        let h = hook.clone();
        self.on_cron_tick.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_cron_tick(ctx).await })
        }));

        let h = hook.clone();
        self.on_cron_job_scheduled.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_cron_job_scheduled(ctx).await })
        }));

        let h = hook.clone();
        self.on_local_queue_init.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_local_queue_init(ctx).await })
        }));

        let h = hook.clone();
        self.on_local_queue_set_mode.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_local_queue_set_mode(ctx).await })
        }));

        let h = hook.clone();
        self.on_local_queue_get_jobs_complete
            .push(Box::new(move |ctx| {
                let h = h.clone();
                Box::pin(async move { h.on_local_queue_get_jobs_complete(ctx).await })
            }));

        let h = hook.clone();
        self.on_local_queue_return_jobs.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.on_local_queue_return_jobs(ctx).await })
        }));

        let h = hook.clone();
        self.on_local_queue_refetch_delay_start
            .push(Box::new(move |ctx| {
                let h = h.clone();
                Box::pin(async move { h.on_local_queue_refetch_delay_start(ctx).await })
            }));

        let h = hook.clone();
        self.on_local_queue_refetch_delay_abort
            .push(Box::new(move |ctx| {
                let h = h.clone();
                Box::pin(async move { h.on_local_queue_refetch_delay_abort(ctx).await })
            }));

        let h = hook.clone();
        self.on_local_queue_refetch_delay_expired
            .push(Box::new(move |ctx| {
                let h = h.clone();
                Box::pin(async move { h.on_local_queue_refetch_delay_expired(ctx).await })
            }));

        let h = hook.clone();
        self.before_job_run.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.before_job_run(ctx).await })
        }));

        let h = hook.clone();
        self.after_job_run.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.after_job_run(ctx).await })
        }));

        let h = hook;
        self.before_job_schedule.push(Box::new(move |ctx| {
            let h = h.clone();
            Box::pin(async move { h.before_job_schedule(ctx).await })
        }));
    }

    pub fn emit_local_queue_init(&self, ctx: LocalQueueInitContext) {
        for hook in &self.on_local_queue_init {
            tokio::spawn(hook(ctx.clone()));
        }
    }

    pub fn emit_local_queue_set_mode(&self, ctx: LocalQueueSetModeContext) {
        for hook in &self.on_local_queue_set_mode {
            tokio::spawn(hook(ctx.clone()));
        }
    }

    pub fn emit_local_queue_get_jobs_complete(&self, ctx: LocalQueueGetJobsCompleteContext) {
        for hook in &self.on_local_queue_get_jobs_complete {
            tokio::spawn(hook(ctx.clone()));
        }
    }

    pub fn emit_local_queue_return_jobs(&self, ctx: LocalQueueReturnJobsContext) {
        for hook in &self.on_local_queue_return_jobs {
            tokio::spawn(hook(ctx.clone()));
        }
    }

    pub fn emit_local_queue_refetch_delay_start(&self, ctx: LocalQueueRefetchDelayStartContext) {
        for hook in &self.on_local_queue_refetch_delay_start {
            tokio::spawn(hook(ctx.clone()));
        }
    }

    pub fn emit_local_queue_refetch_delay_abort(&self, ctx: LocalQueueRefetchDelayAbortContext) {
        for hook in &self.on_local_queue_refetch_delay_abort {
            tokio::spawn(hook(ctx.clone()));
        }
    }

    pub fn emit_local_queue_refetch_delay_expired(
        &self,
        ctx: LocalQueueRefetchDelayExpiredContext,
    ) {
        for hook in &self.on_local_queue_refetch_delay_expired {
            tokio::spawn(hook(ctx.clone()));
        }
    }
}
