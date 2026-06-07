mod state;
mod waiter;

use std::sync::atomic::{AtomicUsize, Ordering};

use event_listener::{Event, EventListener};

use state::{
    notify_broadcast, notify_has_permit, notify_pending, notify_waiters, notify_with_broadcast,
    notify_with_pending, notify_with_waiters, NOTIFY_COUNTER_MASK, NOTIFY_PENDING, NOTIFY_PERMIT,
    NOTIFY_WAITER,
};

pub use waiter::Notified;

#[derive(Default)]
pub struct Notify {
    pub(super) state: AtomicUsize,
    pub(super) event: Event,
}

impl Notify {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify_one(&self) {
        if self.mark_one_notified() {
            self.event.notify_additional(1);
        }
    }

    pub fn notify_waiters(&self) {
        self.mark_all_notified();
        self.event.notify(usize::MAX);
    }

    pub fn notified(&self) -> Notified<'_> {
        Notified::new(self, notify_broadcast(self.state.load(Ordering::Acquire)))
    }

    fn mark_one_notified(&self) -> bool {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let waiters = notify_waiters(state);
            let pending = notify_pending(state);

            let new_state = if waiters > pending {
                state + NOTIFY_PENDING
            } else {
                state | NOTIFY_PERMIT
            };

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return waiters > pending;
            }
        }
    }

    fn mark_all_notified(&self) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let broadcast = notify_broadcast(state);
            let new_broadcast = (broadcast + 1) & NOTIFY_COUNTER_MASK;
            let new_state = notify_with_waiters(
                notify_with_pending(notify_with_broadcast(state, new_broadcast), 0),
                0,
            );

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
        }
    }

    pub(super) fn register_waiter(&self, broadcast: usize) -> Option<EventListener> {
        loop {
            let state = self.state.load(Ordering::Acquire);
            if notify_broadcast(state) != broadcast {
                return None;
            }

            if notify_has_permit(state) {
                let new_state = state & !NOTIFY_PERMIT;
                if self
                    .state
                    .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return None;
                }
                continue;
            }

            let waiters = notify_waiters(state);
            assert!(waiters < NOTIFY_COUNTER_MASK, "too many notify waiters");

            let listener = self.event.listen();
            if self
                .state
                .compare_exchange_weak(
                    state,
                    state + NOTIFY_WAITER,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Some(listener);
            }
        }
    }
}
