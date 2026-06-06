use std::sync::Mutex;

use graphile_worker_runtime as runtime;
use tracing::warn;

pub(crate) struct BackgroundTasks {
    name: &'static str,
    handles: Vec<runtime::JoinHandle<()>>,
}

impl BackgroundTasks {
    pub(crate) fn new(name: &'static str) -> Self {
        Self {
            name,
            handles: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, handle: runtime::JoinHandle<()>) {
        self.handles.push(handle);
    }

    fn abort(&self) {
        for handle in &self.handles {
            handle.abort_handle().abort();
        }
    }

    pub(crate) async fn stop(mut self) {
        self.abort();

        let handles = std::mem::take(&mut self.handles);
        for handle in handles {
            match handle.await {
                Ok(()) | Err(runtime::JoinError::Aborted) => {}
                Err(error) => {
                    warn!(task_group = self.name, error = %error, "Background task failed during shutdown")
                }
            }
        }
    }
}

impl Drop for BackgroundTasks {
    fn drop(&mut self) {
        self.abort();
    }
}

pub(crate) struct TaskSlot {
    name: &'static str,
    handle: Mutex<Option<runtime::JoinHandle<()>>>,
}

impl TaskSlot {
    pub(crate) fn empty(name: &'static str) -> Self {
        Self {
            name,
            handle: Mutex::new(None),
        }
    }

    pub(crate) fn new(name: &'static str, handle: runtime::JoinHandle<()>) -> Self {
        Self {
            name,
            handle: Mutex::new(Some(handle)),
        }
    }

    pub(crate) fn replace_abort(&self, handle: runtime::JoinHandle<()>) {
        if let Some(existing) = self
            .handle
            .lock()
            .expect("task slot poisoned")
            .replace(handle)
        {
            existing.abort_handle().abort();
        }
    }

    pub(crate) fn abort(&self) {
        if let Some(handle) = self.handle.lock().expect("task slot poisoned").take() {
            handle.abort_handle().abort();
        }
    }

    pub(crate) async fn stop(&self) {
        let handle = self.handle.lock().expect("task slot poisoned").take();
        let Some(handle) = handle else {
            return;
        };

        match handle.await {
            Ok(()) | Err(runtime::JoinError::Aborted) => {}
            Err(error) => {
                warn!(task = self.name, error = %error, "Background task failed during shutdown")
            }
        }
    }
}

impl Drop for TaskSlot {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.lock().expect("task slot poisoned").take() {
            handle.abort_handle().abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn task_slot_stop_waits_for_task_completion() {
        let completed = Arc::new(AtomicBool::new(false));
        let completed_for_task = completed.clone();
        let slot = TaskSlot::new(
            "test_slot",
            runtime::spawn(async move {
                completed_for_task.store(true, Ordering::SeqCst);
            }),
        );

        slot.stop().await;

        assert!(completed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn background_tasks_stop_aborts_pending_tasks() {
        let completed = Arc::new(AtomicBool::new(false));
        let completed_for_task = completed.clone();

        let mut tasks = BackgroundTasks::new("test_tasks");
        tasks.push(runtime::spawn(async move {
            runtime::sleep(Duration::from_secs(60)).await;
            completed_for_task.store(true, Ordering::SeqCst);
        }));

        tasks.stop().await;

        assert!(!completed.load(Ordering::SeqCst));
    }
}
