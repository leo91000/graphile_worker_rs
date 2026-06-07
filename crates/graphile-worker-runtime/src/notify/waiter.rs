use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};

use event_listener::EventListener;

use super::state::{notify_broadcast, notify_pending, notify_waiters, notify_with_pending};
use super::{Notify, NOTIFY_WAITER};

struct NotifyWaiter<'a> {
    notify: &'a Notify,
    broadcast: usize,
    active: bool,
}

impl NotifyWaiter<'_> {
    fn try_complete(&mut self) -> bool {
        loop {
            let state = self.notify.state.load(Ordering::Acquire);
            let current_broadcast = notify_broadcast(state);
            let pending = notify_pending(state);

            if current_broadcast != self.broadcast {
                self.active = false;
                return true;
            }

            if pending == 0 {
                return false;
            }

            let waiters = notify_waiters(state);
            if waiters == 0 {
                self.active = false;
                return true;
            }

            let new_pending = pending - 1;
            let new_state =
                notify_with_pending(state - NOTIFY_WAITER, new_pending.min(waiters - 1));

            if self
                .notify
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.active = false;
                return true;
            }
        }
    }
}

impl Drop for NotifyWaiter<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        let notify_another = loop {
            let state = self.notify.state.load(Ordering::Acquire);
            if notify_broadcast(state) != self.broadcast {
                break false;
            }

            let waiters = notify_waiters(state);
            if waiters == 0 {
                break false;
            }

            let pending = notify_pending(state);
            let new_waiters = waiters - 1;
            let new_pending = pending.min(new_waiters);
            let new_state = notify_with_pending(state - NOTIFY_WAITER, new_pending);

            if self
                .notify
                .state
                .compare_exchange_weak(state, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break pending > 0 && new_waiters > 0;
            }
        };

        if notify_another {
            self.notify.event.notify_additional(1);
        }
    }
}

pub struct Notified<'a> {
    notify: &'a Notify,
    broadcast: usize,
    waiter: Option<NotifyWaiter<'a>>,
    listener: Option<EventListener>,
}

impl<'a> Notified<'a> {
    pub(super) fn new(notify: &'a Notify, broadcast: usize) -> Self {
        Self {
            notify,
            broadcast,
            waiter: None,
            listener: None,
        }
    }
}

impl Future for Notified<'_> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().get_mut();

        loop {
            if this.waiter.is_none() {
                let listener = match this.notify.register_waiter(this.broadcast) {
                    Some(listener) => listener,
                    None => return Poll::Ready(()),
                };

                this.waiter = Some(NotifyWaiter {
                    notify: this.notify,
                    broadcast: this.broadcast,
                    active: true,
                });
                this.listener = Some(listener);
            }

            if this.waiter.as_mut().expect("notify waiter").try_complete() {
                this.waiter = None;
                this.listener = None;
                return Poll::Ready(());
            }

            let listener = this.listener.as_mut().expect("notify listener");
            if Pin::new(listener).poll(cx).is_pending() {
                return Poll::Pending;
            }

            if this.waiter.as_mut().expect("notify waiter").try_complete() {
                this.waiter = None;
                this.listener = None;
                return Poll::Ready(());
            }

            this.listener = Some(this.notify.event.listen());
        }
    }
}
