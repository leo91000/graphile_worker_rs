use std::any::{Any, TypeId};
use std::collections::HashMap;

use futures::future::BoxFuture;

use crate::event::{Event, Interceptable};
use crate::events::Emittable;

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

    pub async fn intercept<C: Interceptable>(&self, ctx: C) -> C::Output {
        ctx.intercept_with(self).await
    }
}
