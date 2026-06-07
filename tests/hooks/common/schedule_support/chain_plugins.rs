#[derive(Debug, Default)]
struct ChainedTransformCounters {
    plugin1_calls: AtomicU32,
    plugin2_calls: AtomicU32,
}

#[derive(Clone)]
struct ChainedTransformPlugin1 {
    counters: Arc<ChainedTransformCounters>,
}

impl Plugin for ChainedTransformPlugin1 {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.plugin1_calls.fetch_add(1, Ordering::SeqCst);
                let mut payload = ctx.payload.clone();
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("plugin1_processed".into(), serde_json::json!(true));
                }
                JobScheduleResult::Continue(payload)
            }
        });
    }
}

#[derive(Clone)]
struct ChainedTransformPlugin2 {
    counters: Arc<ChainedTransformCounters>,
}

impl Plugin for ChainedTransformPlugin2 {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.plugin2_calls.fetch_add(1, Ordering::SeqCst);
                let mut payload = ctx.payload.clone();
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("plugin2_processed".into(), serde_json::json!(true));
                }
                JobScheduleResult::Continue(payload)
            }
        });
    }
}


#[derive(Clone)]
struct SkippingFirstPlugin;

impl Plugin for SkippingFirstPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(BeforeJobSchedule, move |ctx| async move {
            let should_skip = ctx
                .payload
                .get("skip_in_first")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if should_skip {
                return JobScheduleResult::Skip;
            }
            JobScheduleResult::Continue(ctx.payload)
        });
    }
}

#[derive(Clone)]
struct SecondPluginCounter {
    calls: Arc<AtomicU32>,
}

impl Plugin for SecondPluginCounter {
    fn register(self, hooks: &mut HookRegistry) {
        let calls = self.calls.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let calls = calls.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                JobScheduleResult::Continue(ctx.payload)
            }
        });
    }
}
