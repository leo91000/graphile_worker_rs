use std::future::Future;

use futures::future::BoxFuture;

use crate::event::Event;
use crate::plugin::Plugin;
use crate::TypeErasedHooks;

#[derive(Default)]
pub struct HookRegistry {
    pub(crate) inner: TypeErasedHooks,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on<E, F, Fut>(&mut self, _event: E, handler: F) -> &mut Self
    where
        E: Event,
        F: Fn(E::Context) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = E::Output> + Send + 'static,
    {
        let boxed: Box<dyn Fn(E::Context) -> BoxFuture<'static, E::Output> + Send + Sync> =
            Box::new(move |ctx| {
                let fut = handler(ctx);
                Box::pin(fut)
            });
        E::register_boxed(&mut self.inner, boxed);
        self
    }

    pub fn with_plugin<P: Plugin>(mut self, plugin: P) -> Self {
        plugin.register(&mut self);
        self
    }
}
