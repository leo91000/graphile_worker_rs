mod context;
mod erased;
mod event;
mod events;
mod plugin;
mod registry;
mod result;

pub use context::*;
pub use erased::TypeErasedHooks;
pub use event::{Event, HookOutput, Interceptable};
pub use events::*;
pub use plugin::Plugin;
pub use registry::HookRegistry;
pub use result::*;

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use graphile_worker_job::Job;
    use serde_json::json;

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

    #[test]
    fn hook_output_defaults_and_registry_registration_paths() {
        assert!(!<() as HookOutput>::is_terminal(&()));
        assert_eq!(<() as HookOutput>::chain_value(&()), None);
        let _: () = <() as HookOutput>::default_with_value(());

        assert!(!HookResult::Continue.is_terminal());
        assert!(HookResult::Skip.is_terminal());
        assert!(HookResult::Fail("failed".to_string()).is_terminal());

        let value = json!({ "ok": true });
        let schedule_result = JobScheduleResult::Continue(value.clone());
        assert!(!schedule_result.is_terminal());
        assert_eq!(schedule_result.chain_value(), Some(value.clone()));
        assert!(JobScheduleResult::Skip.is_terminal());
        assert!(JobScheduleResult::Fail("failed".to_string()).is_terminal());
        match JobScheduleResult::default_with_value(value.clone()) {
            JobScheduleResult::Continue(actual) => assert_eq!(actual, value),
            _ => panic!("expected continue"),
        }

        let mut registry = HookRegistry::new();
        registry.on(JobComplete, |_ctx| async {});
        assert!(!registry.is_empty());
    }
}
