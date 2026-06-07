pub(super) const NOTIFY_COUNTER_BITS: usize = (usize::BITS as usize - 1) / 3;
pub(super) const NOTIFY_COUNTER_MASK: usize = (1 << NOTIFY_COUNTER_BITS) - 1;
pub(super) const NOTIFY_PERMIT: usize = 1;
pub(super) const NOTIFY_WAITERS_SHIFT: usize = 1;
pub(super) const NOTIFY_PENDING_SHIFT: usize = NOTIFY_WAITERS_SHIFT + NOTIFY_COUNTER_BITS;
pub(super) const NOTIFY_BROADCAST_SHIFT: usize = NOTIFY_PENDING_SHIFT + NOTIFY_COUNTER_BITS;
pub(super) const NOTIFY_WAITER: usize = 1 << NOTIFY_WAITERS_SHIFT;
pub(super) const NOTIFY_PENDING: usize = 1 << NOTIFY_PENDING_SHIFT;

pub(super) fn notify_has_permit(state: usize) -> bool {
    state & NOTIFY_PERMIT != 0
}

pub(super) fn notify_waiters(state: usize) -> usize {
    (state >> NOTIFY_WAITERS_SHIFT) & NOTIFY_COUNTER_MASK
}

pub(super) fn notify_pending(state: usize) -> usize {
    (state >> NOTIFY_PENDING_SHIFT) & NOTIFY_COUNTER_MASK
}

pub(super) fn notify_broadcast(state: usize) -> usize {
    (state >> NOTIFY_BROADCAST_SHIFT) & NOTIFY_COUNTER_MASK
}

pub(super) fn notify_with_pending(state: usize, pending: usize) -> usize {
    let cleared = state & !(NOTIFY_COUNTER_MASK << NOTIFY_PENDING_SHIFT);
    cleared | (pending << NOTIFY_PENDING_SHIFT)
}

pub(super) fn notify_with_waiters(state: usize, waiters: usize) -> usize {
    let cleared = state & !(NOTIFY_COUNTER_MASK << NOTIFY_WAITERS_SHIFT);
    cleared | (waiters << NOTIFY_WAITERS_SHIFT)
}

pub(super) fn notify_with_broadcast(state: usize, broadcast: usize) -> usize {
    let cleared = state & !(NOTIFY_COUNTER_MASK << NOTIFY_BROADCAST_SHIFT);
    cleared | (broadcast << NOTIFY_BROADCAST_SHIFT)
}
