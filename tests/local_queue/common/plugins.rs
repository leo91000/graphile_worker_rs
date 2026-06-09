use super::*;

#[derive(Clone)]
pub(super) struct LocalQueueInitCounterPlugin {
    pub(super) counter: Arc<AtomicU32>,
}

impl Plugin for LocalQueueInitCounterPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(LocalQueueInit, move |_ctx| {
            let counter = self.counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}

#[derive(Clone)]
pub(super) struct LocalQueueGetJobsCompleteCounterPlugin {
    pub(super) counter: Arc<AtomicU32>,
}

impl Plugin for LocalQueueGetJobsCompleteCounterPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(LocalQueueGetJobsComplete, move |_ctx| {
            let counter = self.counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}
