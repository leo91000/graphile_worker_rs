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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_details_new_creates_empty() {
        let details = TaskDetails::new();
        assert!(details.task_ids().is_empty());
        assert!(details.task_names().is_empty());
    }

    #[test]
    fn task_details_insert_and_get() {
        let mut details = TaskDetails::new();
        details.insert(1, "task_one".to_string());
        details.insert(2, "task_two".to_string());

        assert_eq!(details.get(&1), Some(&"task_one".to_string()));
        assert_eq!(details.get(&2), Some(&"task_two".to_string()));
        assert_eq!(details.get(&3), None);
    }

    #[test]
    fn task_details_task_ids_and_names() {
        let mut details = TaskDetails::new();
        details.insert(10, "alpha".to_string());
        details.insert(20, "beta".to_string());

        let ids = details.task_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&10));
        assert!(ids.contains(&20));

        let names = details.task_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
    }

    #[test]
    fn task_details_get_or_empty_found() {
        let mut details = TaskDetails::new();
        details.insert(5, "my_task".to_string());

        let result = details.get_or_empty(&100, &5);
        assert_eq!(result, "my_task");
    }

    #[test]
    fn task_details_get_or_empty_not_found() {
        let details = TaskDetails::new();
        let result = details.get_or_empty(&100, &999);
        assert_eq!(result, "");
    }

    #[test]
    fn task_details_default() {
        let details = TaskDetails::default();
        assert!(details.task_ids().is_empty());
    }

    #[tokio::test]
    async fn shared_task_details_new_and_default() {
        let shared = SharedTaskDetails::new(TaskDetails::new());
        assert!(shared.task_ids().await.is_empty());

        let shared_default = SharedTaskDetails::default();
        assert!(shared_default.task_ids().await.is_empty());
    }

    #[tokio::test]
    async fn shared_task_details_from_task_details() {
        let mut details = TaskDetails::new();
        details.insert(1, "test".to_string());

        let shared: SharedTaskDetails = details.into();
        assert_eq!(shared.get(&1).await, Some("test".to_string()));
    }

    #[tokio::test]
    async fn shared_task_details_insert_and_get() {
        let shared = SharedTaskDetails::default();
        shared.insert(42, "answer".to_string()).await;

        assert_eq!(shared.get(&42).await, Some("answer".to_string()));
        assert_eq!(shared.get(&0).await, None);
    }

    #[tokio::test]
    async fn shared_task_details_task_ids_and_names() {
        let shared = SharedTaskDetails::default();
        shared.insert(1, "one".to_string()).await;
        shared.insert(2, "two".to_string()).await;

        let ids = shared.task_ids().await;
        assert_eq!(ids.len(), 2);

        let names = shared.task_names().await;
        assert_eq!(names.len(), 2);
    }

    #[tokio::test]
    async fn shared_task_details_get_or_empty() {
        let shared = SharedTaskDetails::default();
        shared.insert(5, "found".to_string()).await;

        assert_eq!(shared.get_or_empty(&1, &5).await, "found");
        assert_eq!(shared.get_or_empty(&1, &999).await, "");
    }

    #[tokio::test]
    async fn shared_task_details_read_write() {
        let shared = SharedTaskDetails::default();

        {
            let mut guard = shared.write().await;
            guard.insert(100, "via_write".to_string());
        }

        {
            let guard = shared.read().await;
            assert_eq!(guard.get(&100), Some(&"via_write".to_string()));
        }
    }
}
