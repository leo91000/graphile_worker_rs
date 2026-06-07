use futures::future::BoxFuture;

use crate::context::{
    CronJobScheduledContext, CronTickContext, JobCompleteContext, JobFailContext, JobFetchContext,
    JobInterruptedContext, JobPermanentlyFailContext, JobStartContext,
    LocalQueueGetJobsCompleteContext, LocalQueueInitContext, LocalQueueRefetchDelayAbortContext,
    LocalQueueRefetchDelayExpiredContext, LocalQueueRefetchDelayStartContext,
    LocalQueueReturnJobsContext, LocalQueueSetModeContext, WorkerInitContext,
    WorkerRecoveredContext, WorkerShutdownContext, WorkerStartContext,
};
use crate::event::Event;
use crate::events::Emittable;
use crate::TypeErasedHooks;

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

define_observer_event!(WorkerInit, WorkerInitContext);
define_observer_event!(WorkerStart, WorkerStartContext);
define_observer_event!(WorkerShutdown, WorkerShutdownContext);
define_observer_event!(JobFetch, JobFetchContext);
define_observer_event!(JobStart, JobStartContext);
define_observer_event!(JobComplete, JobCompleteContext);
define_observer_event!(JobFail, JobFailContext);
define_observer_event!(JobPermanentlyFail, JobPermanentlyFailContext);
define_observer_event!(JobInterrupted, JobInterruptedContext);
define_observer_event!(WorkerRecovered, WorkerRecoveredContext);
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
