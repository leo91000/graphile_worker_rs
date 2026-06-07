use futures::future::BoxFuture;

use crate::context::{
    AfterJobRunContext, BeforeJobRunContext, BeforeJobScheduleContext, JobRecoveryContext,
};
use crate::event::{Event, HookOutput, Interceptable};
use crate::result::{HookResult, JobRecoveryResult, JobScheduleResult};
use crate::TypeErasedHooks;

macro_rules! define_interceptor_event {
    ($event:ident, $context:ty) => {
        define_interceptor_event!($event, $context, HookResult);
    };

    ($event:ident, $context:ty, $output:ty) => {
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
                        return <$output as Default>::default();
                    };
                    for handler in handlers {
                        let result = handler(self.clone()).await;
                        if result.is_terminal() {
                            return result;
                        }
                    }
                    <$output as Default>::default()
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

define_interceptor_event!(BeforeJobRun, BeforeJobRunContext);
define_interceptor_event!(AfterJobRun, AfterJobRunContext);
define_interceptor_event!(
    BeforeJobSchedule,
    BeforeJobScheduleContext,
    JobScheduleResult,
    payload
);
define_interceptor_event!(JobRecovery, JobRecoveryContext, JobRecoveryResult);
