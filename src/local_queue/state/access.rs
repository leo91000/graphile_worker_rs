use std::sync::atomic::Ordering;

use super::super::LocalQueue;

impl LocalQueue {
    pub(in crate::local_queue) fn try_start_fetch(&self) -> bool {
        self.0
            .fetch_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub(in crate::local_queue) fn end_fetch(&self) {
        self.0.fetch_in_progress.store(false, Ordering::SeqCst);
    }

    pub(in crate::local_queue) fn is_fetch_in_progress(&self) -> bool {
        self.0.fetch_in_progress.load(Ordering::SeqCst)
    }

    pub(in crate::local_queue) fn set_fetch_again(&self, value: bool) {
        self.0.fetch_again.store(value, Ordering::SeqCst);
    }

    pub(in crate::local_queue) fn take_fetch_again(&self) -> bool {
        self.0.fetch_again.swap(false, Ordering::SeqCst)
    }

    pub(in crate::local_queue) fn is_refetch_delay_active(&self) -> bool {
        self.0.refetch_delay.active.load(Ordering::SeqCst)
    }

    pub(in crate::local_queue) fn set_refetch_delay_active(&self, value: bool) {
        self.0.refetch_delay.active.store(value, Ordering::SeqCst);
    }

    pub(in crate::local_queue) fn reset_refetch_delay_counter(&self) {
        self.0.refetch_delay.counter.store(0, Ordering::SeqCst);
    }

    pub(in crate::local_queue) fn increment_refetch_delay_counter(&self, count: usize) {
        self.0
            .refetch_delay
            .counter
            .fetch_add(count, Ordering::SeqCst);
    }

    pub(in crate::local_queue) fn get_refetch_delay_counter(&self) -> usize {
        self.0.refetch_delay.counter.load(Ordering::SeqCst)
    }

    pub(in crate::local_queue) fn set_refetch_delay_fetch_on_complete(&self, value: bool) {
        self.0
            .refetch_delay
            .fetch_on_complete
            .store(value, Ordering::SeqCst);
    }

    pub(in crate::local_queue) fn take_refetch_delay_fetch_on_complete(&self) -> bool {
        self.0
            .refetch_delay
            .fetch_on_complete
            .swap(false, Ordering::SeqCst)
    }
}
