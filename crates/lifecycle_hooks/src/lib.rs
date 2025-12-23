mod context;
mod event;
mod events;
mod plugin;
mod registry;
mod result;

use std::any::{Any, TypeId};
use std::collections::HashMap;

use futures::future::BoxFuture;

pub use context::*;
pub use event::{Event, HookOutput};
pub use events::*;
pub use plugin::Plugin;
pub use registry::HookRegistry;
pub use result::*;

use event::InterceptorFn;
pub use event::ObserverFn;

type HandlerVec<Ctx, Out> = Vec<Box<dyn Fn(Ctx) -> BoxFuture<'static, Out> + Send + Sync>>;

#[derive(Default)]
pub(crate) struct TypeErasedHooks {
    handlers: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl TypeErasedHooks {
    pub fn get_handlers<E: Event>(&self) -> Option<&HandlerVec<E::Context, E::Output>> {
        self.handlers
            .get(&TypeId::of::<E>())
            .and_then(|h| h.downcast_ref())
    }

    pub fn get_handlers_mut<E: Event>(&mut self) -> &mut HandlerVec<E::Context, E::Output> {
        self.handlers
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::new(HandlerVec::<E::Context, E::Output>::new()))
            .downcast_mut()
            .expect("Handler type mismatch")
    }

    pub async fn emit<C: Emittable>(&self, ctx: C) {
        ctx.emit_to(self).await
    }
}

impl HookRegistry {
    #[doc(hidden)]
    pub async fn emit<C: Emittable>(&self, ctx: C) {
        self.inner.emit(ctx).await
    }

    pub fn before_job_run_handlers(&self) -> &[InterceptorFn<BeforeJobRunContext>] {
        self.inner
            .get_handlers::<BeforeJobRun>()
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn after_job_run_handlers(&self) -> &[InterceptorFn<AfterJobRunContext>] {
        self.inner
            .get_handlers::<AfterJobRun>()
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn before_job_schedule_handlers(
        &self,
    ) -> &[Box<
        dyn Fn(BeforeJobScheduleContext) -> BoxFuture<'static, JobScheduleResult> + Send + Sync,
    >] {
        self.inner
            .get_handlers::<BeforeJobSchedule>()
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn cron_tick_handlers(&self) -> &[ObserverFn<CronTickContext>] {
        self.inner
            .get_handlers::<CronTick>()
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn cron_job_scheduled_handlers(&self) -> &[ObserverFn<CronJobScheduledContext>] {
        self.inner
            .get_handlers::<CronJobScheduled>()
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
