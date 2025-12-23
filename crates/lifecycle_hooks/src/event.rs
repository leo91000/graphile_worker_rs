use futures::future::BoxFuture;

use crate::result::{HookResult, JobScheduleResult};
use crate::TypeErasedHooks;

pub trait HookOutput: Default + Send + 'static {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl HookOutput for () {}

impl HookOutput for HookResult {
    fn is_terminal(&self) -> bool {
        !matches!(self, HookResult::Continue)
    }
}

impl HookOutput for JobScheduleResult {
    fn is_terminal(&self) -> bool {
        !matches!(self, JobScheduleResult::Continue(_))
    }
}

#[allow(private_interfaces)]
pub trait Event: Send + Sync + 'static {
    type Context: Clone + Send + 'static;
    type Output: HookOutput;

    #[doc(hidden)]
    fn register_boxed(
        hooks: &mut TypeErasedHooks,
        handler: Box<dyn Fn(Self::Context) -> BoxFuture<'static, Self::Output> + Send + Sync>,
    );
}

pub type ObserverFn<Ctx> = Box<dyn Fn(Ctx) -> BoxFuture<'static, ()> + Send + Sync>;
pub(crate) type InterceptorFn<Ctx> =
    Box<dyn Fn(Ctx) -> BoxFuture<'static, HookResult> + Send + Sync>;
