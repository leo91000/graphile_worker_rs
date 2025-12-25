use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

#[derive(Debug, Clone, Default)]
pub struct TaskDetails(HashMap<i32, String>);

impl TaskDetails {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn task_ids(&self) -> Vec<i32> {
        self.0.keys().copied().collect()
    }

    pub fn task_names(&self) -> Vec<String> {
        self.0.values().cloned().collect()
    }

    pub fn get(&self, id: &i32) -> Option<&String> {
        self.0.get(id)
    }

    pub fn get_or_empty(&self, job_id: &i64, task_id: &i32) -> String {
        match self.0.get(task_id) {
            Some(identifier) => identifier.to_owned(),
            None => {
                warn!(
                    job_id,
                    task_id, "Unknown task_id for job, using empty task identifier"
                );
                String::new()
            }
        }
    }

    pub fn insert(&mut self, id: i32, identifier: String) {
        self.0.insert(id, identifier);
    }
}

#[derive(Debug, Clone)]
pub struct SharedTaskDetails(Arc<RwLock<TaskDetails>>);

impl SharedTaskDetails {
    pub fn new(details: TaskDetails) -> Self {
        Self(Arc::new(RwLock::new(details)))
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, TaskDetails> {
        self.0.read().await
    }

    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, TaskDetails> {
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
