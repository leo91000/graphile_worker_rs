use std::collections::HashMap;

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

    pub fn get_id(&self, identifier: &str) -> Option<i32> {
        self.0
            .iter()
            .find_map(|(id, value)| (value == identifier).then_some(*id))
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
