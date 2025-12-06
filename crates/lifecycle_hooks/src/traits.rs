use std::future::Future;

use crate::context::{
    AfterJobRunContext, BeforeJobRunContext, BeforeJobScheduleContext, CronJobScheduledContext,
    CronTickContext, JobCompleteContext, JobFailContext, JobFetchContext,
    JobPermanentlyFailContext, JobStartContext, WorkerInitContext, WorkerShutdownContext,
    WorkerStartContext,
};
use crate::result::{HookResult, JobScheduleResult};

pub trait LifecycleHooks: Send + Sync + 'static {
    fn on_worker_init(&self, _ctx: WorkerInitContext) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_worker_start(&self, _ctx: WorkerStartContext) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_worker_shutdown(&self, _ctx: WorkerShutdownContext) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_job_fetch(&self, _ctx: JobFetchContext) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_job_start(&self, _ctx: JobStartContext) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_job_complete(&self, _ctx: JobCompleteContext) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_job_fail(&self, _ctx: JobFailContext) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_job_permanently_fail(
        &self,
        _ctx: JobPermanentlyFailContext,
    ) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_cron_tick(&self, _ctx: CronTickContext) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn on_cron_job_scheduled(
        &self,
        _ctx: CronJobScheduledContext,
    ) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn before_job_run(&self, _ctx: BeforeJobRunContext) -> impl Future<Output = HookResult> + Send {
        async { HookResult::Continue }
    }

    fn after_job_run(&self, _ctx: AfterJobRunContext) -> impl Future<Output = HookResult> + Send {
        async { HookResult::Continue }
    }

    fn before_job_schedule(
        &self,
        ctx: BeforeJobScheduleContext,
    ) -> impl Future<Output = JobScheduleResult> + Send {
        async { JobScheduleResult::Continue(ctx.payload) }
    }
}
