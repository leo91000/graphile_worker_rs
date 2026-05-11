use crate::HookRegistry;

pub trait Plugin: Send + Sync + 'static {
    fn register(self, hooks: &mut HookRegistry);
}
