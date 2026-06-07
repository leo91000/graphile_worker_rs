use std::task::Poll;

use crate::Notify;
use futures::{executor::block_on, FutureExt};

#[test]
fn notify_one_coalesces_pending_permits() {
    block_on(async {
        let notify = Notify::new();
        notify.notify_one();
        notify.notify_one();
        notify.notified().await;

        let pending = notify.notified().fuse();
        futures::pin_mut!(pending);

        assert!(matches!(futures::poll!(pending.as_mut()), Poll::Pending));
    });
}

#[test]
fn notify_waiters_wakes_all_registered_waiters() {
    block_on(async {
        let notify = Notify::new();
        let first = notify.notified().fuse();
        let second = notify.notified().fuse();
        futures::pin_mut!(first, second);

        assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
        assert!(matches!(futures::poll!(second.as_mut()), Poll::Pending));

        notify.notify_waiters();

        assert!(matches!(futures::poll!(first.as_mut()), Poll::Ready(())));
        assert!(matches!(futures::poll!(second.as_mut()), Poll::Ready(())));
    });
}

#[test]
fn notify_waiters_does_not_store_a_permit() {
    block_on(async {
        let notify = Notify::new();
        notify.notify_waiters();

        let pending = notify.notified().fuse();
        futures::pin_mut!(pending);

        assert!(matches!(futures::poll!(pending.as_mut()), Poll::Pending));
    });
}

#[test]
fn notify_waiters_wakes_futures_created_before_first_poll() {
    block_on(async {
        let notify = Notify::new();
        let pending = notify.notified().fuse();
        futures::pin_mut!(pending);

        notify.notify_waiters();

        assert!(matches!(futures::poll!(pending.as_mut()), Poll::Ready(())));
    });
}

#[test]
fn notify_one_after_notify_waiters_stores_permit() {
    block_on(async {
        let notify = Notify::new();
        let first = notify.notified().fuse();
        let second = notify.notified().fuse();
        futures::pin_mut!(first, second);

        assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
        assert!(matches!(futures::poll!(second.as_mut()), Poll::Pending));

        notify.notify_waiters();
        notify.notify_one();

        assert!(matches!(futures::poll!(first.as_mut()), Poll::Ready(())));
        assert!(matches!(futures::poll!(second.as_mut()), Poll::Ready(())));

        let permit = notify.notified().fuse();
        futures::pin_mut!(permit);
        assert!(matches!(futures::poll!(permit.as_mut()), Poll::Ready(())));
    });
}

#[test]
fn notify_one_wakes_one_registered_waiter_per_call() {
    block_on(async {
        let notify = Notify::new();
        let first = notify.notified().fuse();
        let second = notify.notified().fuse();
        futures::pin_mut!(first, second);

        assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
        assert!(matches!(futures::poll!(second.as_mut()), Poll::Pending));

        notify.notify_one();

        let first_ready = matches!(futures::poll!(first.as_mut()), Poll::Ready(()));
        let second_ready = matches!(futures::poll!(second.as_mut()), Poll::Ready(()));
        assert_eq!(usize::from(first_ready) + usize::from(second_ready), 1);

        notify.notify_one();

        if !first_ready {
            assert!(matches!(futures::poll!(first.as_mut()), Poll::Ready(())));
        }
        if !second_ready {
            assert!(matches!(futures::poll!(second.as_mut()), Poll::Ready(())));
        }
    });
}
