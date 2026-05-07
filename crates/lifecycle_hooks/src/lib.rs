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
pub use event::{Event, HookOutput, Interceptable};
pub use events::*;
pub use plugin::Plugin;
pub use registry::HookRegistry;
pub use result::*;

type HandlerVec<Ctx, Out> = Vec<Box<dyn Fn(Ctx) -> BoxFuture<'static, Out> + Send + Sync>>;

#[derive(Default)]
#[doc(hidden)]
pub struct TypeErasedHooks {
    handlers: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl TypeErasedHooks {
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

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
        if self.is_empty() {
            return;
        }

        ctx.emit_to(self).await
    }

    pub async fn intercept<C: event::Interceptable>(&self, ctx: C) -> C::Output {
        ctx.intercept_with(self).await
    }
}

impl HookRegistry {
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub async fn emit<C: Emittable>(&self, ctx: C) {
        self.inner.emit(ctx).await
    }

    pub async fn intercept<C: event::Interceptable>(&self, ctx: C) -> C::Output {
        self.inner.intercept(ctx).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use graphile_worker_job::Job;

    use super::*;

    #[test]
    fn emit_on_empty_registry_returns() {
        futures::executor::block_on(async {
            TypeErasedHooks::default()
                .emit(JobCompleteContext {
                    job: Arc::new(Job::builder().build()),
                    worker_id: "worker".to_string(),
                    duration: Duration::ZERO,
                })
                .await;
        });
    }
}
