#[derive(Debug, Default)]
struct ScheduleHookCounters {
    before_schedule: AtomicU32,
    transformed: AtomicU32,
    skipped: AtomicU32,
    failed: AtomicU32,
}

#[derive(Clone)]
struct ScheduleHooksPlugin {
    counters: Arc<ScheduleHookCounters>,
}

impl ScheduleHooksPlugin {
    fn new() -> Self {
        Self {
            counters: Arc::new(ScheduleHookCounters::default()),
        }
    }

    fn counters(&self) -> Arc<ScheduleHookCounters> {
        self.counters.clone()
    }
}

impl Plugin for ScheduleHooksPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        let counters = self.counters.clone();
        hooks.on(BeforeJobSchedule, move |ctx| {
            let counters = counters.clone();
            async move {
                counters.before_schedule.fetch_add(1, Ordering::SeqCst);

                let should_skip = ctx
                    .payload
                    .get("skip_schedule")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let should_fail = ctx
                    .payload
                    .get("fail_schedule")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let should_transform = ctx
                    .payload
                    .get("transform")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if should_skip {
                    counters.skipped.fetch_add(1, Ordering::SeqCst);
                    return JobScheduleResult::Skip;
                }

                if should_fail {
                    counters.failed.fetch_add(1, Ordering::SeqCst);
                    return JobScheduleResult::Fail("Schedule failed by hook".into());
                }

                if should_transform {
                    counters.transformed.fetch_add(1, Ordering::SeqCst);
                    let mut new_payload = ctx.payload.clone();
                    if let Some(obj) = new_payload.as_object_mut() {
                        obj.insert("transformed".into(), serde_json::json!(true));
                    }
                    return JobScheduleResult::Continue(new_payload);
                }

                JobScheduleResult::Continue(ctx.payload)
            }
        });
    }
}
