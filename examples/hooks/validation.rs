use graphile_worker::{BeforeJobRun, HookRegistry, HookResult, Plugin};

pub(super) struct ValidationPlugin;

impl Plugin for ValidationPlugin {
    fn register(self, hooks: &mut HookRegistry) {
        hooks.on(BeforeJobRun, async |ctx| {
            let should_skip = ctx
                .payload
                .get("skip")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let should_fail = ctx
                .payload
                .get("force_fail")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if should_skip {
                println!(
                    "[ValidationPlugin] Skipping job {} due to skip flag",
                    ctx.job.id()
                );
                return HookResult::Skip;
            }

            if should_fail {
                println!(
                    "[ValidationPlugin] Failing job {} due to force_fail flag",
                    ctx.job.id()
                );
                return HookResult::Fail("Forced failure by validation plugin".into());
            }

            HookResult::Continue
        });
    }
}
