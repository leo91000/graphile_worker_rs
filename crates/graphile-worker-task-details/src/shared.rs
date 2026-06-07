use std::sync::Arc;

use graphile_worker_runtime::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::TaskDetails;

#[derive(Debug, Clone)]
pub struct SharedTaskDetails(Arc<RwLock<TaskDetails>>);

impl SharedTaskDetails {
    pub fn new(details: TaskDetails) -> Self {
        Self(Arc::new(RwLock::new(details)))
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, TaskDetails> {
        self.0.read().await
    }

    pub async fn write(&self) -> RwLockWriteGuard<'_, TaskDetails> {
        self.0.write().await
    }

    pub async fn task_ids(&self) -> Vec<i32> {
        self.0.read().await.task_ids()
    }

    pub async fn task_names(&self) -> Vec<String> {
        self.0.read().await.task_names()
    }

    pub async fn get(&self, id: &i32) -> Option<String> {
        self.0.read().await.get(id).cloned()
    }

    pub async fn get_or_empty(&self, job_id: &i64, task_id: &i32) -> String {
        self.0.read().await.get_or_empty(job_id, task_id)
    }

    pub async fn insert(&self, id: i32, identifier: String) {
        self.0.write().await.insert(id, identifier);
    }
}

impl Default for SharedTaskDetails {
    fn default() -> Self {
        Self::new(TaskDetails::default())
    }
}

impl From<TaskDetails> for SharedTaskDetails {
    fn from(details: TaskDetails) -> Self {
        Self::new(details)
    }
}
