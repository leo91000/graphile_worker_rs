use futures::future::BoxFuture;

use crate::result::{HookResult, JobScheduleResult};
use crate::TypeErasedHooks;

pub trait HookOutput: Default + Send + 'static {
    type ChainValue: Clone + Send;

    fn is_terminal(&self) -> bool {
        false
    }

    fn chain_value(&self) -> Option<Self::ChainValue> {
        None
    }

    fn default_with_value(_value: Self::ChainValue) -> Self {
        Self::default()
    }
}

impl HookOutput for () {
    type ChainValue = ();
}

impl HookOutput for HookResult {
    type ChainValue = ();

    fn is_terminal(&self) -> bool {
        !matches!(self, HookResult::Continue)
    }
}

impl HookOutput for JobScheduleResult {
    type ChainValue = serde_json::Value;

    fn is_terminal(&self) -> bool {
        !matches!(self, JobScheduleResult::Continue(_))
    }

    fn chain_value(&self) -> Option<Self::ChainValue> {
        match self {
            JobScheduleResult::Continue(v) => Some(v.clone()),
            _ => None,
        }
    }

    fn default_with_value(value: Self::ChainValue) -> Self {
        JobScheduleResult::Continue(value)
    }
}

pub trait Event: Send + Sync + 'static {
    type Context: Clone + Send + 'static;
    type Output: HookOutput;

    #[doc(hidden)]
    fn register_boxed(
        hooks: &mut TypeErasedHooks,
        handler: Box<dyn Fn(Self::Context) -> BoxFuture<'static, Self::Output> + Send + Sync>,
    );
}

pub trait Interceptable: Clone + Send + 'static {
    type Output: HookOutput;

    #[doc(hidden)]
    fn intercept_with(self, hooks: &TypeErasedHooks) -> BoxFuture<'_, Self::Output>;
}
