//! Coordinates retries with every local queue using the same worker identity.
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use graphile_worker_runtime as runtime;

/// Allows concurrent fetches, but excludes them while a return is uncertain.
#[derive(Default)]
pub(super) struct ClaimCoordinator {
    fetches: runtime::RwLock<()>,
    returns: AtomicUsize,
}

impl ClaimCoordinator {
    /// Shares coordination across queue instances without retaining dead workers.
    pub(super) fn for_worker(worker_id: &str) -> Arc<Self> {
        static WORKERS: OnceLock<Mutex<HashMap<String, Weak<ClaimCoordinator>>>> = OnceLock::new();
        let mut workers = WORKERS.get_or_init(Default::default).lock().unwrap();
        workers.retain(|_, worker| worker.strong_count() != 0);
        if let Some(worker) = workers.get(worker_id).and_then(Weak::upgrade) {
            return worker;
        }
        let worker = Arc::new(Self::default());
        workers.insert(worker_id.to_owned(), Arc::downgrade(&worker));
        worker
    }

    /// Holds the fetch boundary through SQL; blocked callers can poll or shut down.
    pub(super) fn try_fetch(&self) -> Option<runtime::RwLockReadGuard<'_, ()>> {
        let guard = self.fetches.try_read()?;
        (self.returns.load(Ordering::Acquire) == 0).then_some(guard)
    }

    /// Waits for earlier fetch SQL, then blocks new fetches until acknowledgement.
    pub(super) async fn begin_return(self: &Arc<Self>) -> ReturnPermit {
        let _fetches = self.fetches.write().await;
        self.returns.fetch_add(1, Ordering::AcqRel);
        ReturnPermit(self.clone())
    }
}

/// Lives with the pending claims, including across cancelled return futures.
pub(super) struct ReturnPermit(Arc<ClaimCoordinator>);

impl Drop for ReturnPermit {
    /// Reopens fetching only after every pending return has been acknowledged.
    fn drop(&mut self) {
        self.0.returns.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Owns the claim IDs and their exclusion permit until successful return.
#[derive(Default)]
pub(super) struct PendingReturns {
    pub(super) jobs: Vec<graphile_worker_job::Job>,
    pub(super) permit: Option<ReturnPermit>,
}
