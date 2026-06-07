mod interceptor;
mod observer;

use futures::future::BoxFuture;

use crate::TypeErasedHooks;

#[doc(hidden)]
pub trait Emittable: Clone + Send + 'static {
    #[doc(hidden)]
    fn emit_to(self, hooks: &TypeErasedHooks) -> BoxFuture<'_, ()>;
}

pub use interceptor::*;
pub use observer::*;
