#[derive(Debug, Default)]
struct IdentifierCapturingCounters {
    captured_identifier: std::sync::Mutex<Option<String>>,
    captured_spec_queue: std::sync::Mutex<Option<String>>,
    captured_spec_priority: std::sync::Mutex<Option<i16>>,
}

#[derive(Clone)]
struct IdentifierCapturingPlugin {
    counters: Arc<IdentifierCapturingCounters>,
}

impl IdentifierCapturingPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(IdentifierCapturingCounters::default()),
        }
    }

    fn counters(&self) -> Arc<IdentifierCapturingCounters> {
        self.counters.clone()
    }
}

impl Plugin for IdentifierCapturingPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let counters = counters.clone();
            async move {
                *counters.captured_identifier.lock().unwrap() = Some(ctx.identifier.clone());
                *counters.captured_spec_queue.lock().unwrap() = ctx.spec.queue_name().clone();
                *counters.captured_spec_priority.lock().unwrap() = *ctx.spec.priority();
                JobScheduleResult::Continue(ctx.payload)
            }
        });
    }
}
