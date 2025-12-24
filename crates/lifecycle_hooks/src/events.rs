use futures::future::BoxFuture;

use crate::context::{
    AfterJobRunContext, BeforeJobRunContext, BeforeJobScheduleContext, CronJobScheduledContext,
    CronTickContext, JobCompleteContext, JobFailContext, JobFetchContext,
    JobPermanentlyFailContext, JobStartContext, LocalQueueGetJobsCompleteContext,
    LocalQueueInitContext, LocalQueueRefetchDelayAbortContext,
    LocalQueueRefetchDelayExpiredContext, LocalQueueRefetchDelayStartContext,
    LocalQueueReturnJobsContext, LocalQueueSetModeContext, WorkerInitContext,
    WorkerShutdownContext, WorkerStartContext,
};
use crate::event::{Event, HookOutput, Interceptable};
use crate::result::{HookResult, JobScheduleResult};
use crate::TypeErasedHooks;

#[doc(hidden)]
pub trait Emittable: Clone + Send + 'static {
    #[doc(hidden)]
    fn emit_to(self, hooks: &TypeErasedHooks) -> BoxFuture<'_, ()>;
}

macro_rules! define_observer_event {
    ($event:ident, $context:ty) => {
        pub struct $event;

        impl Event for $event {
            type Context = $context;
            type Output = ();

            fn register_boxed(
                hooks: &mut TypeErasedHooks,
                handler: Box<
                    dyn Fn(Self::Context) -> BoxFuture<'static, Self::Output> + Send + Sync,
                >,
            ) {
                hooks.get_handlers_mut::<Self>().push(handler);
            }
        }

        impl Emittable for $context {
            fn emit_to(self, hooks: &TypeErasedHooks) -> BoxFuture<'_, ()> {
                Box::pin(async move {
                    if let Some(handlers) = hooks.get_handlers::<$event>() {
                        let futures: Vec<_> = handlers.iter().map(|h| h(self.clone())).collect();
                        futures::future::join_all(futures).await;
                    }
                })
            }
        }
    };
}

macro_rules! define_interceptor_event {
    ($event:ident, $context:ty) => {
        pub struct $event;

        impl Event for $event {
            type Context = $context;
            type Output = HookResult;

            fn register_boxed(
                hooks: &mut TypeErasedHooks,
                handler: Box<
                    dyn Fn(Self::Context) -> BoxFuture<'static, Self::Output> + Send + Sync,
                >,
            ) {
                hooks.get_handlers_mut::<Self>().push(handler);
            }
        }

        impl Interceptable for $context {
            type Output = HookResult;

            fn intercept_with(self, hooks: &TypeErasedHooks) -> BoxFuture<'_, Self::Output> {
                Box::pin(async move {
                    let Some(handlers) = hooks.get_handlers::<$event>() else {
                        return HookResult::default();
                    };
                    for handler in handlers {
                        let result = handler(self.clone()).await;
                        if result.is_terminal() {
                            return result;
                        }
                    }
                    HookResult::default()
                })
            }
        }
    };

    ($event:ident, $context:ty, $output:ty, $chain_field:ident) => {
        pub struct $event;

        impl Event for $event {
            type Context = $context;
            type Output = $output;

            fn register_boxed(
                hooks: &mut TypeErasedHooks,
                handler: Box<
                    dyn Fn(Self::Context) -> BoxFuture<'static, Self::Output> + Send + Sync,
                >,
            ) {
                hooks.get_handlers_mut::<Self>().push(handler);
            }
        }

        impl Interceptable for $context {
            type Output = $output;

            fn intercept_with(self, hooks: &TypeErasedHooks) -> BoxFuture<'_, Self::Output> {
                Box::pin(async move {
                    let Some(handlers) = hooks.get_handlers::<$event>() else {
                        return <$output as HookOutput>::default_with_value(self.$chain_field);
                    };

                    let mut current_value = self.$chain_field.clone();
                    for handler in handlers {
                        let mut ctx = self.clone();
                        ctx.$chain_field = current_value;
                        let result = handler(ctx).await;
                        if result.is_terminal() {
                            return result;
                        }
                        current_value = result.chain_value().unwrap();
                    }
                    <$output as HookOutput>::default_with_value(current_value)
                })
            }
        }
    };
}

define_observer_event!(WorkerInit, WorkerInitContext);
define_observer_event!(WorkerStart, WorkerStartContext);
define_observer_event!(WorkerShutdown, WorkerShutdownContext);
define_observer_event!(JobFetch, JobFetchContext);
define_observer_event!(JobStart, JobStartContext);
define_observer_event!(JobComplete, JobCompleteContext);
define_observer_event!(JobFail, JobFailContext);
define_observer_event!(JobPermanentlyFail, JobPermanentlyFailContext);
define_observer_event!(CronTick, CronTickContext);
define_observer_event!(CronJobScheduled, CronJobScheduledContext);
define_observer_event!(LocalQueueInit, LocalQueueInitContext);
define_observer_event!(LocalQueueSetMode, LocalQueueSetModeContext);
define_observer_event!(LocalQueueGetJobsComplete, LocalQueueGetJobsCompleteContext);
define_observer_event!(LocalQueueReturnJobs, LocalQueueReturnJobsContext);
define_observer_event!(
    LocalQueueRefetchDelayStart,
    LocalQueueRefetchDelayStartContext
);
define_observer_event!(
    LocalQueueRefetchDelayAbort,
    LocalQueueRefetchDelayAbortContext
);
define_observer_event!(
    LocalQueueRefetchDelayExpired,
    LocalQueueRefetchDelayExpiredContext
);

define_interceptor_event!(BeforeJobRun, BeforeJobRunContext);
define_interceptor_event!(AfterJobRun, AfterJobRunContext);
define_interceptor_event!(
    BeforeJobSchedule,
    BeforeJobScheduleContext,
    JobScheduleResult,
    payload
);
